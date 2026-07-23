use crate::context::{build_context, render_json as render_context_json, ContextOptions};
use crate::impact;
use crate::memory;
use crate::model::{Error, GraphIndex, Result, SymbolKind};
use crate::search::find_symbols;
use crate::secret::redact_secrets;
use crate::storage;
use crate::util::{index_path, json_escape, normalized_relative};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::time::UNIX_EPOCH;

const MCP_PROTOCOL_VERSION: &str = "2025-11-25";
const MCP_SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] = ["2025-11-25", "2025-06-18", "2024-11-05"];

#[derive(Debug, Default)]
struct SessionState {
    initialized: bool,
}

pub fn serve(root: &Path, mut graph: GraphIndex) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    let mut state = SessionState::default();
    let mut stamp = index_stamp(root);

    for line_result in stdin.lock().lines() {
        let line = line_result?;
        if line.trim().is_empty() {
            continue;
        }

        let current_stamp = index_stamp(root);
        if current_stamp != stamp {
            match storage::load(root) {
                Ok(reloaded) => {
                    graph = reloaded;
                    stamp = current_stamp;
                }
                Err(error) => eprintln!("MCP index reload failed; retaining previous snapshot: {error}"),
            }
        }

        let request_id = extract_raw_id(&line);
        let response = match handle_message(root, &graph, &mut state, &line) {
            Ok(Some(response)) => response,
            Ok(None) => continue,
            Err(error) => {
                let Some(id) = request_id.as_deref() else {
                    eprintln!("MCP notification rejected: {error}");
                    continue;
                };
                error_response(id, -32603, &error.to_string())
            }
        };
        stdout.write_all(response.as_bytes())?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }
    Ok(())
}

fn index_stamp(root: &Path) -> Option<(u64, u128)> {
    let metadata = fs::metadata(index_path(root)).ok()?;
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some((metadata.len(), modified))
}

fn handle_message(
    root: &Path,
    graph: &GraphIndex,
    state: &mut SessionState,
    input: &str,
) -> Result<Option<String>> {
    let method = extract_json_string(input, "method")
        .ok_or_else(|| Error::Protocol("missing JSON-RPC method".to_string()))?;
    let request_id = extract_raw_id(input);
    let id = request_id.as_deref().unwrap_or("null");
    match method.as_str() {
        "notifications/initialized" | "notifications/cancelled" => Ok(None),
        "ping" => Ok(if request_id.is_some() {
            Some(success_response(id, "{}"))
        } else {
            None
        }),
        "initialize" => {
            if request_id.is_none() {
                return Err(Error::Protocol("initialize must be a request with an id".to_string()));
            }
            let requested = extract_json_string(input, "protocolVersion")
                .unwrap_or_else(|| MCP_PROTOCOL_VERSION.to_string());
            let negotiated = MCP_SUPPORTED_PROTOCOL_VERSIONS
                .iter()
                .copied()
                .find(|version| *version == requested.as_str())
                .unwrap_or(MCP_PROTOCOL_VERSION);
            state.initialized = true;
            Ok(Some(success_response(
                id,
                &format!(
                    "{{\"protocolVersion\":\"{negotiated}\",\"capabilities\":{{\"tools\":{{\"listChanged\":false}}}},\"serverInfo\":{{\"name\":\"codespace\",\"version\":\"{}\"}},\"instructions\":\"Use cse_context first for compact task context, cse_search for symbol discovery, cse_impact before risky edits, and cse_history for prior decisions.\"}}",
                    env!("CARGO_PKG_VERSION")
                ),
            )))
        }
        "tools/list" => {
            require_initialized(state)?;
            Ok(if request_id.is_some() {
                Some(success_response(id, &tools_list_json()))
            } else {
                None
            })
        }
        "tools/call" => {
            require_initialized(state)?;
            if request_id.is_none() {
                return Err(Error::Protocol("tools/call must include an id".to_string()));
            }
            let name = required_json_string(input, "name")?;
            let (text, is_error) = match call_tool(root, graph, &name, input) {
                Ok(text) => (text, false),
                Err(error) => (error.to_string(), true),
            };
            Ok(Some(success_response(
                id,
                &format!(
                    "{{\"content\":[{{\"type\":\"text\",\"text\":\"{}\"}}],\"isError\":{is_error}}}",
                    json_escape(&text)
                ),
            )))
        }
        _ if request_id.is_none() => Ok(None),
        _ => Ok(Some(error_response(id, -32601, "method not found"))),
    }
}

fn require_initialized(state: &SessionState) -> Result<()> {
    if state.initialized {
        Ok(())
    } else {
        Err(Error::Protocol("server has not completed initialization".to_string()))
    }
}

fn required_json_string(input: &str, key: &str) -> Result<String> {
    extract_json_string(input, key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| Error::InvalidArgument(format!("missing required string `{key}`")))
}

fn call_tool(root: &Path, graph: &GraphIndex, name: &str, input: &str) -> Result<String> {
    match name {
        "cse_search" => {
            let query = required_json_string(input, "query")?;
            let kind = match extract_json_string(input, "kind") {
                Some(value) => Some(SymbolKind::parse(&value).ok_or_else(|| {
                    Error::InvalidArgument(format!("unknown symbol kind `{value}`"))
                })?),
                None => None,
            };
            let limit = extract_json_number(input, "limit").unwrap_or(20).clamp(1, 200) as usize;
            let hits = find_symbols(graph, &query, kind, limit);
            let mut rows = Vec::new();
            for hit in &hits {
                let Some(symbol) = graph.symbols.get(&hit.symbol_id) else {
                    continue;
                };
                let path = graph.file_for_symbol(symbol).map_or("", |file| file.path.as_str());
                rows.push(format!(
                    "{{\"id\":{},\"name\":\"{}\",\"qualified_name\":\"{}\",\"kind\":\"{}\",\"path\":\"{}\",\"line_start\":{},\"line_end\":{},\"score_milli\":{},\"reasons\":[{}]}}",
                    symbol.id,
                    json_escape(&symbol.name),
                    json_escape(&symbol.qualified_name),
                    symbol.kind.as_str(),
                    json_escape(path),
                    symbol.line_start,
                    symbol.line_end,
                    hit.score_milli,
                    hit.reasons
                        .iter()
                        .map(|reason| format!("\"{}\"", json_escape(reason)))
                        .collect::<Vec<_>>()
                        .join(",")
                ));
            }
            Ok(format!("[{}]", rows.join(",")))
        }
        "cse_context" => {
            let query = required_json_string(input, "query")?;
            let mut options = ContextOptions::default();
            options.max_tokens = extract_json_number(input, "max_tokens")
                .unwrap_or(options.max_tokens as i64)
                .clamp(128, 32_000) as usize;
            options.max_items = extract_json_number(input, "max_items")
                .unwrap_or(options.max_items as i64)
                .clamp(1, 50) as usize;
            let bundle = build_context(root, graph, &query, &options)?;
            Ok(render_context_json(&bundle))
        }
        "cse_impact" => {
            let from = extract_json_string(input, "from").unwrap_or_else(|| "HEAD~1".to_string());
            let to = extract_json_string(input, "to").unwrap_or_else(|| "HEAD".to_string());
            let depth = extract_json_number(input, "depth").unwrap_or(3).clamp(1, 10) as usize;
            let report = impact::analyze(root, graph, &from, &to, depth)?;
            Ok(impact::render_json(&report))
        }
        "cse_history" => {
            let target = extract_json_string(input, "target").unwrap_or_default();
            let limit = extract_json_number(input, "limit").unwrap_or(10).clamp(1, 100) as usize;
            let decisions = memory::history(graph, &target, limit);
            Ok(memory::render_history_json(&decisions))
        }
        "cse_read" => {
            let file = required_json_string(input, "file")?;
            let max_lines = extract_json_number(input, "max_lines").unwrap_or(400).clamp(1, 5_000) as usize;
            safe_read(root, &file, max_lines)
        }
        _ => Err(Error::InvalidArgument(format!("unknown tool: {name}"))),
    }
}

fn safe_read(root: &Path, requested: &str, max_lines: usize) -> Result<String> {
    let path = root.join(requested);
    let canonical = fs::canonicalize(&path)?;
    let relative = normalized_relative(root, &canonical)?;
    if relative == ".git"
        || relative == ".codespace"
        || relative.starts_with(".git/")
        || relative.starts_with(".codespace/")
    {
        return Err(Error::InvalidArgument("reading internal metadata is blocked".to_string()));
    }
    let metadata = fs::metadata(&canonical)?;
    if !metadata.is_file() {
        return Err(Error::InvalidArgument("requested path is not a regular file".to_string()));
    }
    if metadata.len() > 2_097_152 {
        return Err(Error::InvalidArgument("file exceeds 2 MiB read limit".to_string()));
    }
    let bytes = fs::read(canonical)?;
    let content = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = content.lines().collect();
    let mut output = String::new();
    for (index, line) in lines.iter().take(max_lines).enumerate() {
        output.push_str(&format!("{:>5} | {}\n", index + 1, line.trim_end()));
    }
    if lines.len() > max_lines {
        output.push_str(&format!("... truncated after {max_lines} lines\n"));
    }
    let redacted = redact_secrets(&output);
    Ok(format!(
        "file: {relative}\nredactions: {}\n{}",
        redacted.redactions, redacted.content
    ))
}

fn tools_list_json() -> String {
    r#"{"tools":[
{"name":"cse_search","description":"Search indexed symbols, files, and graph relationships.","inputSchema":{"type":"object","properties":{"query":{"type":"string"},"kind":{"type":"string","enum":["function","method","class","struct","enum","trait","interface","module","constant","variable","type_alias","test"]},"limit":{"type":"integer","minimum":1,"maximum":200}},"required":["query"],"additionalProperties":false}},
{"name":"cse_context","description":"Return a ranked, token-budgeted, secret-redacted code context bundle.","inputSchema":{"type":"object","properties":{"query":{"type":"string"},"max_tokens":{"type":"integer","minimum":128,"maximum":32000},"max_items":{"type":"integer","minimum":1,"maximum":50}},"required":["query"],"additionalProperties":false}},
{"name":"cse_impact","description":"Analyze the transitive blast radius between two Git refs.","inputSchema":{"type":"object","properties":{"from":{"type":"string"},"to":{"type":"string"},"depth":{"type":"integer","minimum":1,"maximum":10}},"additionalProperties":false}},
{"name":"cse_history","description":"Read prior engineering decisions by file, symbol, tag, or summary.","inputSchema":{"type":"object","properties":{"target":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":100}},"additionalProperties":false}},
{"name":"cse_read","description":"Read a project file with path confinement, line limits, and secret redaction.","inputSchema":{"type":"object","properties":{"file":{"type":"string"},"max_lines":{"type":"integer","minimum":1,"maximum":5000}},"required":["file"],"additionalProperties":false}}
]}"#.replace('\n', "")
}

fn success_response(id: &str, result: &str) -> String {
    format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{result}}}")
}

fn error_response(id: &str, code: i32, message: &str) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"error\":{{\"code\":{code},\"message\":\"{}\"}}}}",
        json_escape(message)
    )
}

fn extract_raw_id(input: &str) -> Option<String> {
    let position = find_json_key(input, "id")?;
    let mut index = skip_whitespace(input.as_bytes(), position);
    if input.as_bytes().get(index) == Some(&b'"') {
        let (_, end) = parse_json_string_at(input, index)?;
        return Some(input[index..end].to_string());
    }
    let start = index;
    while let Some(byte) = input.as_bytes().get(index) {
        if matches!(*byte, b',' | b'}' | b' ' | b'\r' | b'\n' | b'\t') {
            break;
        }
        index += 1;
    }
    (index > start).then(|| input[start..index].to_string())
}

fn extract_json_string(input: &str, key: &str) -> Option<String> {
    let position = find_json_key(input, key)?;
    let index = skip_whitespace(input.as_bytes(), position);
    parse_json_string_at(input, index).map(|(value, _)| value)
}

fn extract_json_number(input: &str, key: &str) -> Option<i64> {
    let position = find_json_key(input, key)?;
    let mut index = skip_whitespace(input.as_bytes(), position);
    let start = index;
    if input.as_bytes().get(index) == Some(&b'-') {
        index += 1;
    }
    while input.as_bytes().get(index).is_some_and(|byte| byte.is_ascii_digit()) {
        index += 1;
    }
    input[start..index].parse().ok()
}

fn find_json_key(input: &str, key: &str) -> Option<usize> {
    let needle = format!("\"{}\"", key);
    let mut search_start = 0;
    while let Some(relative) = input[search_start..].find(&needle) {
        let key_start = search_start + relative;
        let after_key = key_start + needle.len();
        let mut index = skip_whitespace(input.as_bytes(), after_key);
        if input.as_bytes().get(index) == Some(&b':') {
            index += 1;
            return Some(index);
        }
        search_start = after_key;
    }
    None
}

fn skip_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while bytes
        .get(index)
        .is_some_and(|byte| matches!(*byte, b' ' | b'\r' | b'\n' | b'\t'))
    {
        index += 1;
    }
    index
}

fn parse_json_string_at(input: &str, start: usize) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut output = String::new();
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => return Some((output, index + 1)),
            b'\\' => {
                index += 1;
                let escaped = *bytes.get(index)?;
                match escaped {
                    b'"' => output.push('"'),
                    b'\\' => output.push('\\'),
                    b'/' => output.push('/'),
                    b'b' => output.push('\u{0008}'),
                    b'f' => output.push('\u{000c}'),
                    b'n' => output.push('\n'),
                    b'r' => output.push('\r'),
                    b't' => output.push('\t'),
                    b'u' => {
                        let end = index + 5;
                        let hex = input.get(index + 1..end)?;
                        let high = u16::from_str_radix(hex, 16).ok()?;
                        if (0xd800..=0xdbff).contains(&high) {
                            if bytes.get(end) != Some(&b'\\') || bytes.get(end + 1) != Some(&b'u') {
                                return None;
                            }
                            let low_end = end + 6;
                            let low_hex = input.get(end + 2..low_end)?;
                            let low = u16::from_str_radix(low_hex, 16).ok()?;
                            if !(0xdc00..=0xdfff).contains(&low) {
                                return None;
                            }
                            let scalar = 0x1_0000
                                + ((u32::from(high) - 0xd800) << 10)
                                + (u32::from(low) - 0xdc00);
                            output.push(char::from_u32(scalar)?);
                            index = low_end - 1;
                        } else if (0xdc00..=0xdfff).contains(&high) {
                            return None;
                        } else {
                            output.push(char::from_u32(u32::from(high))?);
                            index = end - 1;
                        }
                    }
                    _ => return None,
                }
            }
            byte if byte < 0x80 => output.push(char::from(byte)),
            _ => {
                let remaining = input.get(index..)?;
                let character = remaining.chars().next()?;
                output.push(character);
                index += character.len_utf8() - 1;
            }
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_fields() {
        let input = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"cse_context","arguments":{"query":"login bug","max_tokens":900}}}"#;
        assert_eq!(extract_raw_id(input).as_deref(), Some("7"));
        assert_eq!(extract_json_string(input, "method").as_deref(), Some("tools/call"));
        assert_eq!(extract_json_string(input, "query").as_deref(), Some("login bug"));
        assert_eq!(extract_json_number(input, "max_tokens"), Some(900));
    }

    #[test]
    fn parses_surrogate_pair_escape() {
        let input = r#"{"query":"fix \ud83d\udd27"}"#;
        assert_eq!(extract_json_string(input, "query").as_deref(), Some("fix 🔧"));
    }

    #[test]
    fn requires_initialization_before_tool_listing() {
        let graph = GraphIndex::empty(".".to_string(), 0);
        let mut state = SessionState::default();
        let input = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
        assert!(handle_message(Path::new("."), &graph, &mut state, input).is_err());
    }
}
