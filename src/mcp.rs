use crate::context::{ContextOptions, build_context, render_markdown};
use crate::impact;
use crate::memory;
use crate::model::{Error, GraphIndex, Result, SymbolKind};
use crate::search::find_symbols;
use crate::storage;
use crate::util::{index_path, json_escape};
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
        if serde_json::from_str::<serde_json::Value>(&line).is_err() {
            stdout.write_all(error_response("null", -32700, "parse error").as_bytes())?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
            continue;
        }

        let current_stamp = index_stamp(root);
        if current_stamp != stamp {
            match storage::load(root) {
                Ok(reloaded) => {
                    graph = reloaded;
                    stamp = current_stamp;
                }
                Err(error) => {
                    eprintln!("MCP index reload failed; retaining previous snapshot: {error}")
                }
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
                error_response(id, -32600, &error.to_string())
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
    let parsed: serde_json::Value = serde_json::from_str(input)
        .map_err(|error| Error::Protocol(format!("invalid JSON-RPC: {error}")))?;
    if parsed.get("jsonrpc").and_then(serde_json::Value::as_str) != Some("2.0") {
        return Err(Error::Protocol("jsonrpc must be 2.0".to_string()));
    }
    let params = parsed
        .get("params")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}))
        .to_string();
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
                return Err(Error::Protocol(
                    "initialize must be a request with an id".to_string(),
                ));
            }
            let requested = extract_json_string(&params, "protocolVersion")
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
            let name = required_json_string(&params, "name")?;
            let parameters: serde_json::Value = serde_json::from_str(&params)
                .map_err(|error| Error::Protocol(error.to_string()))?;
            let arguments = parameters
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let result = validate_arguments(&name, &arguments)
                .and_then(|()| call_tool(root, graph, &name, &arguments.to_string()));
            let (text, is_error) = match result {
                Ok(text) => (text, false),
                Err(error) => (error.to_string(), true),
            };
            let mut result =
                serde_json::json!({"content":[{"type":"text","text":text}],"isError":is_error});
            if !is_error {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                    if value.is_object() {
                        result["structuredContent"] = value;
                    }
                }
            }
            Ok(Some(success_response(id, &result.to_string())))
        }
        _ if request_id.is_none() => Ok(None),
        _ => Ok(Some(error_response(id, -32601, "method not found"))),
    }
}

fn require_initialized(state: &SessionState) -> Result<()> {
    if state.initialized {
        Ok(())
    } else {
        Err(Error::Protocol(
            "server has not completed initialization".to_string(),
        ))
    }
}

fn required_json_string(input: &str, key: &str) -> Result<String> {
    extract_json_string(input, key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| Error::InvalidArgument(format!("missing required string `{key}`")))
}

fn call_tool(root: &Path, graph: &GraphIndex, name: &str, input: &str) -> Result<String> {
    match name {
        "cse_status" => Ok(serde_json::json!({"version": env!("CARGO_PKG_VERSION"), "index_revision": graph.index_revision,
            "files": graph.files.len(), "symbols": graph.symbols.len(), "edges": graph.edges.len(),
            "decisions": graph.decisions.len()}).to_string()),
        "cse_bulk_read" => {
            let input = crate::agent_runtime::parse_json(input)?;
            Ok(crate::agent_runtime::execute(root, "bulk-read", &input)?.to_string())
        }
        "cse_run" => {
            let mut input = crate::agent_runtime::parse_json(input)?;
            let operation = input.get("operation").and_then(serde_json::Value::as_str)
                .ok_or_else(|| Error::InvalidArgument("run operation is required".to_string()))?.to_string();
            if let Some(object) = input.as_object_mut() { object.remove("operation"); }
            Ok(crate::agent_runtime::execute(root, &format!("run-{operation}"), &input)?.to_string())
        }
        "cse_search" => {
            let query = required_json_string(input, "query")?;
            let kind = match extract_json_string(input, "kind") {
                Some(value) => Some(SymbolKind::parse(&value).ok_or_else(|| {
                    Error::InvalidArgument(format!("unknown symbol kind `{value}`"))
                })?),
                None => None,
            };
            let limit = extract_json_number(input, "limit")
                .unwrap_or(20)
                .clamp(1, 200) as usize;
            let hits = find_symbols(graph, &query, kind, limit);
            let mut rows = Vec::new();
            for hit in &hits {
                let Some(symbol) = graph.symbols.get(&hit.symbol_id) else {
                    continue;
                };
                let path = graph
                    .file_for_symbol(symbol)
                    .map_or("", |file| file.path.as_str());
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
        "cse_context" | "cse_context_pack" => {
            let query = required_json_string(input, "query")?;
            let mut options = ContextOptions::default();
            options.max_tokens = extract_json_number(input, "max_tokens")
                .unwrap_or(options.max_tokens as i64)
                .clamp(128, 32_000) as usize;
            options.max_items = extract_json_number(input, "max_items")
                .unwrap_or(options.max_items as i64)
                .clamp(1, 50) as usize;
            let bundle = build_context(root, graph, &query, &options)?;
            if name == "cse_context_pack" { Ok(crate::context::render_json(&bundle)) }
            else { Ok(render_markdown(&bundle)) }
        }
        "cse_impact" => {
            let from = extract_json_string(input, "from").unwrap_or_else(|| "HEAD~1".to_string());
            let to = extract_json_string(input, "to").unwrap_or_else(|| "HEAD".to_string());
            let depth = extract_json_number(input, "depth")
                .unwrap_or(3)
                .clamp(1, 10) as usize;
            let report = impact::analyze(root, graph, &from, &to, depth)?;
            Ok(impact::render_json(&report))
        }
        "cse_history" => {
            let target = extract_json_string(input, "target").unwrap_or_default();
            let limit = extract_json_number(input, "limit")
                .unwrap_or(10)
                .clamp(1, 100) as usize;
            let decisions = memory::history(graph, &target, limit);
            Ok(memory::render_history_json(&decisions))
        }
        "cse_read" => {
            let file = required_json_string(input, "file")?;
            let max_lines = extract_json_number(input, "max_lines")
                .unwrap_or(400)
                .clamp(1, 5_000) as usize;
            let start_line = extract_json_number(input, "start_line").unwrap_or(1);
            if start_line < 1 { return Err(Error::InvalidArgument("start_line must be positive".to_string())); }
            safe_read(root, &file, start_line as usize, max_lines)
        }
        "cse_task_add" => {
            let title = required_json_string(input, "title")?;
            let description = extract_json_string(input, "description").unwrap_or_default();
            let priority = extract_json_string(input, "priority")
                .and_then(|p| crate::tasks::TaskPriority::parse(&p))
                .unwrap_or(crate::tasks::TaskPriority::Medium);
            let due = extract_json_number(input, "due").map(|v| v as u128);
            let tags: Vec<String> = extract_json_string(input, "tags")
                .map(|t| t.split(',').map(String::from).collect())
                .unwrap_or_default();
            let mut board = crate::tasks::transaction(root)?;
            let task = board.add(&title, &description, priority, due, tags);
            let task_id = task.id.clone();
            let task_title = task.title.clone();
            crate::tasks::save_tasks(root, &board)?;
            Ok(format!("Created task: {task_id} - {task_title}"))
        }
        "cse_task_list" => {
            let board = crate::tasks::load_tasks(root);
            Ok(board.to_json())
        }
        "cse_task_remove" => {
            let id = required_json_string(input, "id")?;
            let mut board = crate::tasks::transaction(root)?;
            board.remove(&id)?;
            crate::tasks::save_tasks(root, &board)?;
            Ok(format!("Removed task: {id}"))
        }
        "cse_task_status" => {
            let id = required_json_string(input, "id")?;
            let status_str = required_json_string(input, "status")?;
            let status = crate::tasks::TaskStatus::parse(&status_str)
                .ok_or_else(|| Error::InvalidArgument(format!("invalid status: {status_str}")))?;
            let mut board = crate::tasks::transaction(root)?;
            board.set_status(&id, status)?;
            crate::tasks::save_tasks(root, &board)?;
            Ok(format!("Updated task {id} -> {}", status.as_str()))
        }
        "cse_github_status" => {
            let config = crate::github_integration::status(root);
            Ok(config.to_json())
        }
        "cse_github_issues" => {
            let state = extract_json_string(input, "state").unwrap_or_else(|| "open".to_string());
            let owner = extract_json_string(input, "owner");
            let repo = extract_json_string(input, "repo");
            crate::github_integration::list_issues(root, owner.as_deref(), repo.as_deref(), &state)
        }
        _ => Err(Error::InvalidArgument(format!("unknown tool: {name}"))),
    }
}

fn safe_read(root: &Path, requested: &str, start_line: usize, max_lines: usize) -> Result<String> {
    Ok(crate::routing::read_source(root, requested, start_line, max_lines)?.render_plain())
}

fn tools_list_json() -> String {
    let mut catalog: serde_json::Value = serde_json::from_str(r#"{"tools":[
{"name":"cse_search","description":"Search indexed symbols, files, and graph relationships.","inputSchema":{"type":"object","properties":{"query":{"type":"string"},"kind":{"type":"string","enum":["function","method","class","struct","enum","trait","interface","module","constant","variable","type_alias","test"]},"limit":{"type":"integer","minimum":1,"maximum":200}},"required":["query"],"additionalProperties":false}},
{"name":"cse_context","description":"Return a ranked, token-budgeted, secret-redacted code context bundle.","inputSchema":{"type":"object","properties":{"query":{"type":"string"},"max_tokens":{"type":"integer","minimum":128,"maximum":32000},"max_items":{"type":"integer","minimum":1,"maximum":50}},"required":["query"],"additionalProperties":false}},
{"name":"cse_impact","description":"Analyze the transitive blast radius between two Git refs.","inputSchema":{"type":"object","properties":{"from":{"type":"string"},"to":{"type":"string"},"depth":{"type":"integer","minimum":1,"maximum":10}},"additionalProperties":false}},
{"name":"cse_history","description":"Read prior engineering decisions by file, symbol, tag, or summary.","inputSchema":{"type":"object","properties":{"target":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":100}},"additionalProperties":false}},
{"name":"cse_read","description":"Read exact source lines before editing. For large multi-file discovery use cse_bulk_read first.","inputSchema":{"type":"object","properties":{"file":{"type":"string"},"start_line":{"type":"integer","minimum":1},"max_lines":{"type":"integer","minimum":1,"maximum":5000}},"required":["file"],"additionalProperties":false}},
{"name":"cse_bulk_read","description":"Route large repository reads into a bounded, local evidence response with exact source lines. No remote model required. Debug/security reasoning remains with the caller.","inputSchema":{"type":"object","properties":{"question":{"type":"string"},"paths":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":32},"max_tokens":{"type":"integer","minimum":128,"maximum":32000},"task_kind":{"type":"string","enum":["inspect","overview","debug","security","architecture"]},"start_line":{"type":"integer","minimum":1},"max_lines":{"type":"integer","minimum":1,"maximum":5000}},"required":["question","paths"],"additionalProperties":false}},
{"name":"cse_run","description":"Persist a bounded task state and resume from immutable specification, current state and latest observation. Patch uses expected_revision to reject stale writers. No commands are executed.","inputSchema":{"type":"object","properties":{"operation":{"type":"string","enum":["start","show","patch","resume","list"]},"id":{"type":"string"},"initial":{"type":"object"},"expected_revision":{"type":"integer","minimum":0},"patch":{"type":"object"}},"required":["operation"],"additionalProperties":false}},
{"name":"cse_task_add","description":"Add a project task with priority, due date, and tags.","inputSchema":{"type":"object","properties":{"title":{"type":"string"},"description":{"type":"string"},"priority":{"type":"string","enum":["low","medium","high","critical"]},"due":{"type":"integer"},"tags":{"type":"string"}},"required":["title"],"additionalProperties":false}},
{"name":"cse_task_list","description":"List all project tasks as JSON.","inputSchema":{"type":"object","properties":{},"additionalProperties":false}},
{"name":"cse_task_remove","description":"Remove a task by ID.","inputSchema":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"],"additionalProperties":false}},
{"name":"cse_task_status","description":"Update task status.","inputSchema":{"type":"object","properties":{"id":{"type":"string"},"status":{"type":"string","enum":["todo","in_progress","done","cancelled"]}},"required":["id","status"],"additionalProperties":false}},
{"name":"cse_github_status","description":"Check GitHub integration link status.","inputSchema":{"type":"object","properties":{},"additionalProperties":false}},
{"name":"cse_github_issues","description":"List GitHub issues for a repository.","inputSchema":{"type":"object","properties":{"state":{"type":"string","enum":["open","closed","all"]},"owner":{"type":"string"},"repo":{"type":"string"}},"additionalProperties":false}}
]}"#).unwrap_or_else(|_| serde_json::json!({"tools":[]}));
    if let Some(tools) = catalog["tools"].as_array_mut() {
        let mut pack = tools[1].clone();
        pack["name"] = serde_json::json!("cse_context_pack");
        pack["description"] = serde_json::json!(
            "Inspect a Context Pack as structured JSON. For a bounded model prompt use cse_context."
        );
        tools.push(pack);
        tools.push(serde_json::json!({"name":"cse_status","description":"Read the selected repository's index counts and CodeSpace version; does not expose absolute paths.",
            "inputSchema":{"type":"object","properties":{},"additionalProperties":false}}));
        for tool in tools {
            let name = tool["name"].as_str().unwrap_or_default();
            let read_only = !matches!(
                name,
                "cse_run" | "cse_task_add" | "cse_task_status" | "cse_task_remove"
            );
            tool["annotations"] = serde_json::json!({"readOnlyHint":read_only,"destructiveHint":name=="cse_task_remove","idempotentHint":read_only,"openWorldHint":name.starts_with("cse_github")});
        }
    }
    catalog.to_string()
}

fn validate_arguments(name: &str, input: &serde_json::Value) -> Result<()> {
    let catalog: serde_json::Value = serde_json::from_str(&tools_list_json())
        .map_err(|error| Error::Protocol(error.to_string()))?;
    let tool = catalog["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|tool| tool["name"] == name))
        .ok_or_else(|| Error::InvalidArgument(format!("unknown tool `{name}`")))?;
    validate_schema(input, &tool["inputSchema"], "arguments")
}

fn validate_schema(
    value: &serde_json::Value,
    schema: &serde_json::Value,
    field: &str,
) -> Result<()> {
    let bad = || {
        Error::InvalidArgument(format!(
            "invalid `{field}`; must match the tool inputSchema"
        ))
    };
    let valid_type = match schema["type"].as_str() {
        Some("object") => value.is_object(),
        Some("array") => value.is_array(),
        Some("string") => value.is_string(),
        Some("integer") => value.as_i64().is_some(),
        Some("boolean") => value.is_boolean(),
        _ => true,
    };
    if !valid_type {
        return Err(bad());
    }
    if let Some(options) = schema["enum"].as_array() {
        if !options.contains(value) {
            return Err(bad());
        }
    }
    if let Some(n) = value.as_i64() {
        if schema["minimum"].as_i64().is_some_and(|min| n < min)
            || schema["maximum"].as_i64().is_some_and(|max| n > max)
        {
            return Err(bad());
        }
    }
    if let Some(text) = value.as_str() {
        if text.len() > 16_384 {
            return Err(bad());
        }
    }
    if let Some(items) = value.as_array() {
        if schema["minItems"]
            .as_u64()
            .is_some_and(|min| items.len() < min as usize)
            || schema["maxItems"]
                .as_u64()
                .is_some_and(|max| items.len() > max as usize)
        {
            return Err(bad());
        }
        for item in items {
            validate_schema(item, &schema["items"], field)?;
        }
    }
    if let Some(object) = value.as_object() {
        if let Some(required) = schema["required"].as_array() {
            for key in required.iter().filter_map(serde_json::Value::as_str) {
                if !object.contains_key(key) {
                    return Err(bad());
                }
            }
        }
        for (key, value) in object {
            let properties = &schema["properties"];
            if properties.get(key).is_none() && schema["additionalProperties"] == false {
                return Err(bad());
            }
            if let Some(rule) = properties.get(key) {
                validate_schema(value, rule, key)?;
            }
        }
    }
    Ok(())
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
    let value: serde_json::Value = serde_json::from_str(input).ok()?;
    let id = value.get("id")?;
    (id.is_string() || id.is_number() || id.is_null()).then(|| id.to_string())
}

fn extract_json_string(input: &str, key: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(input)
        .ok()?
        .get(key)?
        .as_str()
        .map(str::to_owned)
}

#[cfg(test)]
fn extract_json_object(input: &str, key: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(input).ok()?;
    let object = value.get(key)?;
    object.is_object().then(|| object.to_string())
}

fn extract_json_number(input: &str, key: &str) -> Option<i64> {
    serde_json::from_str::<serde_json::Value>(input)
        .ok()?
        .get(key)?
        .as_i64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_fields() {
        let input = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"cse_context","arguments":{"query":"login bug","max_tokens":900}}}"#;
        assert_eq!(extract_raw_id(input).as_deref(), Some("7"));
        assert_eq!(
            extract_json_string(input, "method").as_deref(),
            Some("tools/call")
        );
        assert_eq!(extract_json_string(input, "query"), None);
        let params = extract_json_object(input, "params").unwrap_or_default();
        let arguments = extract_json_object(&params, "arguments").unwrap_or_default();
        assert_eq!(
            extract_json_string(&arguments, "query").as_deref(),
            Some("login bug")
        );
        assert_eq!(extract_json_number(&arguments, "max_tokens"), Some(900));
        assert_eq!(
            extract_raw_id(r#"{"params":{"id":"forged"},"id":9}"#).as_deref(),
            Some("9")
        );
        assert_eq!(
            extract_json_number(r#"{"max_tokens":1.5}"#, "max_tokens"),
            None
        );
    }

    #[test]
    fn parses_surrogate_pair_escape() {
        let input = r#"{"query":"fix \ud83d\udd27"}"#;
        assert_eq!(
            extract_json_string(input, "query").as_deref(),
            Some("fix 🔧")
        );
    }

    #[test]
    fn requires_initialization_before_tool_listing() {
        let graph = GraphIndex::empty(".".to_string(), 0);
        let mut state = SessionState::default();
        let input = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
        assert!(handle_message(Path::new("."), &graph, &mut state, input).is_err());
    }
}
