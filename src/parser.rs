use crate::model::{Edge, EdgeKind, FileRecord, Symbol, SymbolKind};
use crate::util::{stable_hash, stable_id};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub file: FileRecord,
    pub symbols: Vec<Symbol>,
    pub unresolved_calls: Vec<(u64, String)>,
    pub imports: Vec<String>,
    pub local_edges: Vec<Edge>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum LanguageFamily {
    Rust,
    Python,
    JavaScript,
    Go,
    CLike,
    Ruby,
    Shell,
    Other,
}

pub(crate) fn detect_language(path: &Path) -> Option<(&'static str, LanguageFamily)> {
    let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();
    let result = match extension.as_str() {
        "rs" => ("rust", LanguageFamily::Rust),
        "py" | "pyi" => ("python", LanguageFamily::Python),
        "js" | "jsx" | "mjs" | "cjs" => ("javascript", LanguageFamily::JavaScript),
        "ts" | "tsx" | "mts" | "cts" => ("typescript", LanguageFamily::JavaScript),
        "go" => ("go", LanguageFamily::Go),
        "java" => ("java", LanguageFamily::CLike),
        "kt" | "kts" => ("kotlin", LanguageFamily::CLike),
        "c" | "h" => ("c", LanguageFamily::CLike),
        "cc" | "cpp" | "cxx" | "hpp" | "hh" => ("cpp", LanguageFamily::CLike),
        "cs" => ("csharp", LanguageFamily::CLike),
        "swift" => ("swift", LanguageFamily::CLike),
        "php" => ("php", LanguageFamily::CLike),
        "rb" => ("ruby", LanguageFamily::Ruby),
        "sh" | "bash" | "zsh" => ("shell", LanguageFamily::Shell),
        "lua" => ("lua", LanguageFamily::Other),
        "scala" => ("scala", LanguageFamily::CLike),
        "ex" | "exs" => ("elixir", LanguageFamily::Other),
        "dart" => ("dart", LanguageFamily::CLike),
        "vue" => ("vue", LanguageFamily::JavaScript),
        "svelte" => ("svelte", LanguageFamily::JavaScript),
        "sql" => ("sql", LanguageFamily::Other),
        "proto" => ("protobuf", LanguageFamily::CLike),
        "toml" => ("toml", LanguageFamily::Other),
        "yaml" | "yml" => ("yaml", LanguageFamily::Other),
        "json" => ("json", LanguageFamily::Other),
        "md" | "mdx" => ("markdown", LanguageFamily::Other),
        _ => return None,
    };
    Some(result)
}

pub fn parse_source(
    relative_path: &str,
    absolute_path: &Path,
    source: &str,
    modified_unix_ms: u128,
) -> Option<ParsedFile> {
    let (language, family) = detect_language(absolute_path)?;
    let file_id = stable_id(&["file", relative_path]);
    let lines: Vec<&str> = source.lines().collect();
    let mut symbols: Vec<Symbol> = Vec::new();
    let mut imports = Vec::new();
    let mut unresolved_calls = Vec::new();
    let mut pending_doc = Vec::new();
    let mut pending_test_attribute = false;
    let mut scopes: Vec<(u64, usize, i64)> = Vec::new();
    let mut brace_depth: i64 = 0;
    let mut python_indents: Vec<(u64, usize)> = Vec::new();

    for (zero_index, original_line) in lines.iter().enumerate() {
        let line_number = zero_index + 1;
        let trimmed = original_line.trim();
        if trimmed.is_empty() {
            if !pending_doc.is_empty() {
                pending_doc.push(String::new());
            }
            continue;
        }

        if is_doc_line(trimmed, family) {
            pending_doc.push(clean_doc_line(trimmed, family));
            continue;
        }

        if matches!(family, LanguageFamily::Rust) && trimmed.starts_with("#[") {
            pending_test_attribute |= trimmed.starts_with("#[test]")
                || trimmed.starts_with("#[tokio::test]")
                || trimmed.starts_with("#[async_std::test]");
            continue;
        }

        if let Some(import) = parse_import(trimmed, family) {
            imports.push(import);
        }

        if matches!(family, LanguageFamily::Python) {
            let indent = original_line.len().saturating_sub(original_line.trim_start().len());
            while let Some((symbol_id, previous_indent)) = python_indents.last().copied() {
                if indent > previous_indent || trimmed.starts_with('@') {
                    break;
                }
                python_indents.pop();
                if scopes.last().is_some_and(|(id, _, _)| *id == symbol_id) {
                    scopes.pop();
                }
                if let Some(symbol) = symbols.iter_mut().find(|symbol| symbol.id == symbol_id) {
                    symbol.line_end = line_number.saturating_sub(1).max(symbol.line_start);
                }
            }
        }

        if let Some((name, mut kind, signature)) = parse_declaration(trimmed, family) {
            if matches!(family, LanguageFamily::Rust)
                && pending_test_attribute
                && kind == SymbolKind::Function
            {
                kind = SymbolKind::Test;
            }
            pending_test_attribute = false;
            let parent_name = scopes
                .last()
                .and_then(|(id, _, _)| symbols.iter().find(|symbol| symbol.id == *id))
                .map_or_else(String::new, |symbol| symbol.qualified_name.clone());
            let qualified_name = if parent_name.is_empty() {
                name.clone()
            } else {
                format!("{parent_name}::{name}")
            };
            let id = stable_id(&[
                "symbol",
                relative_path,
                &qualified_name,
                kind.as_str(),
                &line_number.to_string(),
            ]);
            let complexity = estimate_line_complexity(trimmed);
            symbols.push(Symbol {
                id,
                file_id,
                name: name.clone(),
                qualified_name,
                kind,
                line_start: line_number,
                line_end: line_number,
                signature,
                doc: pending_doc.join("\n").trim().to_string(),
                complexity,
            });
            pending_doc.clear();

            match family {
                LanguageFamily::Python => {
                    let indent = original_line.len().saturating_sub(original_line.trim_start().len());
                    python_indents.push((id, indent));
                    scopes.push((id, line_number, i64::try_from(indent).unwrap_or(i64::MAX)));
                }
                _ if opens_scope(trimmed) => {
                    scopes.push((id, line_number, brace_depth));
                }
                _ => {}
            }
        } else {
            pending_doc.clear();
            pending_test_attribute = false;
        }

        let owner_id = current_owner_id(&scopes, &python_indents, family);
        if let Some(owner) = owner_id {
            for call in extract_calls(trimmed, family) {
                if !is_call_keyword(&call) {
                    unresolved_calls.push((owner, call));
                }
            }
            if let Some(symbol) = symbols.iter_mut().find(|symbol| symbol.id == owner) {
                symbol.complexity = symbol
                    .complexity
                    .saturating_add(estimate_line_complexity(trimmed));
            }
        }

        if !matches!(family, LanguageFamily::Python) {
            brace_depth += count_char_outside_strings(trimmed, '{') as i64;
            brace_depth -= count_char_outside_strings(trimmed, '}') as i64;
            while let Some((symbol_id, _, starting_depth)) = scopes.last().copied() {
                if brace_depth > starting_depth {
                    break;
                }
                scopes.pop();
                if let Some(symbol) = symbols.iter_mut().find(|symbol| symbol.id == symbol_id) {
                    symbol.line_end = line_number.max(symbol.line_start);
                }
            }
        }
    }

    let final_line = lines.len().max(1);
    for (symbol_id, _, _) in scopes {
        if let Some(symbol) = symbols.iter_mut().find(|symbol| symbol.id == symbol_id) {
            symbol.line_end = final_line;
        }
    }
    for (symbol_id, _) in python_indents {
        if let Some(symbol) = symbols.iter_mut().find(|symbol| symbol.id == symbol_id) {
            symbol.line_end = final_line;
        }
    }

    let symbol_ids: BTreeSet<u64> = symbols.iter().map(|symbol| symbol.id).collect();
    let mut local_edges = Vec::new();
    for symbol in &symbols {
        local_edges.push(Edge {
            from: file_id,
            to: symbol.id,
            kind: EdgeKind::Contains,
            confidence_milli: 1000,
        });
    }
    for symbol in &symbols {
        let parent = symbols
            .iter()
            .filter(|candidate| {
                candidate.id != symbol.id
                    && candidate.line_start <= symbol.line_start
                    && candidate.line_end >= symbol.line_end
                    && candidate.line_start < symbol.line_start
            })
            .max_by_key(|candidate| candidate.line_start);
        if let Some(parent) = parent {
            if symbol_ids.contains(&parent.id) {
                local_edges.push(Edge {
                    from: parent.id,
                    to: symbol.id,
                    kind: EdgeKind::Contains,
                    confidence_milli: 950,
                });
            }
        }
    }

    Some(ParsedFile {
        file: FileRecord {
            id: file_id,
            path: relative_path.to_string(),
            language: language.to_string(),
            hash: stable_hash(source.as_bytes()),
            bytes: source.len() as u64,
            modified_unix_ms,
            line_count: lines.len(),
        },
        symbols,
        unresolved_calls,
        imports,
        local_edges,
    })
}

pub fn resolve_cross_file_edges(parsed_files: &[ParsedFile]) -> Vec<Edge> {
    let mut edges = Vec::new();
    let mut names: BTreeMap<String, Vec<&Symbol>> = BTreeMap::new();
    let mut paths: BTreeMap<String, u64> = BTreeMap::new();
    for parsed in parsed_files {
        paths.insert(parsed.file.path.clone(), parsed.file.id);
        for symbol in &parsed.symbols {
            names
                .entry(symbol.name.to_ascii_lowercase())
                .or_default()
                .push(symbol);
        }
    }

    for parsed in parsed_files {
        for import in &parsed.imports {
            let normalized = import.replace("::", "/").replace('.', "/");
            let candidate = paths.iter().find(|(path, _)| {
                let without_extension = path.rsplit_once('.').map_or(path.as_str(), |(head, _)| head);
                without_extension.ends_with(&normalized)
                    || without_extension.ends_with(&format!("/{normalized}/mod"))
                    || path.ends_with(&format!("/{normalized}.rs"))
                    || path.ends_with(&format!("/{normalized}.py"))
                    || path.ends_with(&format!("/{normalized}.ts"))
                    || path.ends_with(&format!("/{normalized}.js"))
            });
            if let Some((_, target_file_id)) = candidate {
                edges.push(Edge {
                    from: parsed.file.id,
                    to: *target_file_id,
                    kind: EdgeKind::Imports,
                    confidence_milli: 850,
                });
            }
        }

        for (owner_id, call_name) in &parsed.unresolved_calls {
            let key = call_name.to_ascii_lowercase();
            let Some(candidates) = names.get(&key) else {
                continue;
            };
            let selected = candidates
                .iter()
                .find(|symbol| symbol.file_id == parsed.file.id)
                .copied()
                .or_else(|| (candidates.len() == 1).then_some(candidates[0]));
            if let Some(target) = selected {
                if target.id != *owner_id {
                    edges.push(Edge {
                        from: *owner_id,
                        to: target.id,
                        kind: EdgeKind::Calls,
                        confidence_milli: if target.file_id == parsed.file.id { 900 } else { 700 },
                    });
                }
            }
        }
    }
    edges.sort();
    edges.dedup();
    edges
}

fn parse_declaration(line: &str, family: LanguageFamily) -> Option<(String, SymbolKind, String)> {
    match family {
        LanguageFamily::Rust => parse_rust_declaration(line),
        LanguageFamily::Python => parse_python_declaration(line),
        LanguageFamily::JavaScript => parse_javascript_declaration(line),
        LanguageFamily::Go => parse_go_declaration(line),
        LanguageFamily::CLike => parse_clike_declaration(line),
        LanguageFamily::Ruby => parse_ruby_declaration(line),
        LanguageFamily::Shell => parse_shell_declaration(line),
        LanguageFamily::Other => parse_other_declaration(line),
    }
}

fn parse_rust_declaration(line: &str) -> Option<(String, SymbolKind, String)> {
    let cleaned = strip_visibility(line);
    let patterns = [
        ("async fn ", SymbolKind::Function),
        ("unsafe fn ", SymbolKind::Function),
        ("const fn ", SymbolKind::Function),
        ("fn ", SymbolKind::Function),
        ("struct ", SymbolKind::Struct),
        ("enum ", SymbolKind::Enum),
        ("trait ", SymbolKind::Trait),
        ("mod ", SymbolKind::Module),
        ("type ", SymbolKind::TypeAlias),
        ("const ", SymbolKind::Constant),
        ("static ", SymbolKind::Variable),
    ];
    for (prefix, kind) in patterns {
        if let Some(rest) = cleaned.strip_prefix(prefix) {
            let name = take_identifier(rest)?;
            return Some((name, kind, compact_signature(line)));
        }
    }
    if let Some(rest) = cleaned.strip_prefix("impl ") {
        let name = take_identifier(rest.trim_start_matches('<').trim_start())?;
        return Some((format!("impl_{name}"), SymbolKind::Module, compact_signature(line)));
    }
    None
}

fn parse_python_declaration(line: &str) -> Option<(String, SymbolKind, String)> {
    let cleaned = line.trim_start_matches("async ");
    if let Some(rest) = cleaned.strip_prefix("def ") {
        return Some((take_identifier(rest)?, SymbolKind::Function, compact_signature(line)));
    }
    if let Some(rest) = cleaned.strip_prefix("class ") {
        return Some((take_identifier(rest)?, SymbolKind::Class, compact_signature(line)));
    }
    None
}

fn parse_javascript_declaration(line: &str) -> Option<(String, SymbolKind, String)> {
    let cleaned = line
        .trim_start_matches("export ")
        .trim_start_matches("default ")
        .trim_start_matches("declare ")
        .trim_start_matches("async ");
    for (prefix, kind) in [
        ("function ", SymbolKind::Function),
        ("class ", SymbolKind::Class),
        ("interface ", SymbolKind::Interface),
        ("enum ", SymbolKind::Enum),
        ("type ", SymbolKind::TypeAlias),
        ("namespace ", SymbolKind::Module),
    ] {
        if let Some(rest) = cleaned.strip_prefix(prefix) {
            return Some((take_identifier(rest)?, kind, compact_signature(line)));
        }
    }
    for prefix in ["const ", "let ", "var "] {
        if let Some(rest) = cleaned.strip_prefix(prefix) {
            let name = take_identifier(rest)?;
            if line.contains("=>") || line.contains("function") {
                return Some((name, SymbolKind::Function, compact_signature(line)));
            }
            if prefix == "const " {
                return Some((name, SymbolKind::Constant, compact_signature(line)));
            }
        }
    }
    None
}

fn parse_go_declaration(line: &str) -> Option<(String, SymbolKind, String)> {
    if let Some(rest) = line.strip_prefix("func ") {
        if rest.starts_with('(') {
            let after_receiver = rest.split_once(')')?.1.trim_start();
            return Some((take_identifier(after_receiver)?, SymbolKind::Method, compact_signature(line)));
        }
        return Some((take_identifier(rest)?, SymbolKind::Function, compact_signature(line)));
    }
    if let Some(rest) = line.strip_prefix("type ") {
        let name = take_identifier(rest)?;
        let kind = if rest.contains(" struct") {
            SymbolKind::Struct
        } else if rest.contains(" interface") {
            SymbolKind::Interface
        } else {
            SymbolKind::TypeAlias
        };
        return Some((name, kind, compact_signature(line)));
    }
    None
}

fn parse_clike_declaration(line: &str) -> Option<(String, SymbolKind, String)> {
    let cleaned = line
        .trim_start_matches("public ")
        .trim_start_matches("private ")
        .trim_start_matches("protected ")
        .trim_start_matches("internal ")
        .trim_start_matches("static ")
        .trim_start_matches("final ")
        .trim_start_matches("abstract ");
    for (prefix, kind) in [
        ("class ", SymbolKind::Class),
        ("struct ", SymbolKind::Struct),
        ("enum ", SymbolKind::Enum),
        ("interface ", SymbolKind::Interface),
        ("protocol ", SymbolKind::Interface),
        ("namespace ", SymbolKind::Module),
        ("record ", SymbolKind::Struct),
    ] {
        if let Some(rest) = cleaned.strip_prefix(prefix) {
            return Some((take_identifier(rest)?, kind, compact_signature(line)));
        }
    }
    if line.contains('(')
        && line.contains(')')
        && (line.ends_with('{') || line.ends_with(';') || line.contains(" throws "))
        && !starts_with_control_keyword(cleaned)
    {
        let before_paren = cleaned.split_once('(')?.0.trim_end();
        let name = before_paren
            .split(|character: char| character.is_whitespace() || character == ':' || character == '*')
            .filter(|part| !part.is_empty())
            .next_back()?;
        if is_identifier(name) {
            return Some((name.to_string(), SymbolKind::Function, compact_signature(line)));
        }
    }
    None
}

fn parse_ruby_declaration(line: &str) -> Option<(String, SymbolKind, String)> {
    if let Some(rest) = line.strip_prefix("def ") {
        return Some((take_identifier(rest.trim_start_matches("self."))?, SymbolKind::Function, compact_signature(line)));
    }
    if let Some(rest) = line.strip_prefix("class ") {
        return Some((take_identifier(rest)?, SymbolKind::Class, compact_signature(line)));
    }
    if let Some(rest) = line.strip_prefix("module ") {
        return Some((take_identifier(rest)?, SymbolKind::Module, compact_signature(line)));
    }
    None
}

fn parse_shell_declaration(line: &str) -> Option<(String, SymbolKind, String)> {
    if let Some(rest) = line.strip_prefix("function ") {
        return Some((take_identifier(rest)?, SymbolKind::Function, compact_signature(line)));
    }
    if let Some(name) = line.strip_suffix("() {").or_else(|| line.strip_suffix("(){")) {
        let name = name.trim();
        if is_identifier(name) {
            return Some((name.to_string(), SymbolKind::Function, compact_signature(line)));
        }
    }
    None
}

fn parse_other_declaration(line: &str) -> Option<(String, SymbolKind, String)> {
    if let Some(rest) = line.strip_prefix("defmodule ") {
        return Some((take_identifier(rest)?, SymbolKind::Module, compact_signature(line)));
    }
    if let Some(rest) = line.strip_prefix("def ") {
        return Some((take_identifier(rest)?, SymbolKind::Function, compact_signature(line)));
    }
    if let Some(rest) = line.strip_prefix("function ") {
        return Some((take_identifier(rest)?, SymbolKind::Function, compact_signature(line)));
    }
    None
}

fn parse_import(line: &str, family: LanguageFamily) -> Option<String> {
    let candidate = match family {
        LanguageFamily::Rust => line
            .strip_prefix("use ")
            .or_else(|| line.strip_prefix("mod "))
            .map(|value| value.trim_end_matches(';').split("::{").next().unwrap_or(value)),
        LanguageFamily::Python => {
            if let Some(value) = line.strip_prefix("from ") {
                value.split_whitespace().next()
            } else {
                line.strip_prefix("import ").and_then(|value| value.split_whitespace().next())
            }
        }
        LanguageFamily::JavaScript => {
            if let Some((_, value)) = line.rsplit_once(" from ") {
                Some(value)
            } else if line.starts_with("import ") {
                line.split_whitespace().nth(1)
            } else if let Some((_, value)) = line.split_once("require(") {
                value.split(')').next()
            } else {
                None
            }
        }
        LanguageFamily::Go => {
            if line.starts_with("import ") {
                line.split_whitespace().nth(1)
            } else {
                None
            }
        }
        LanguageFamily::CLike => line
            .strip_prefix("#include ")
            .or_else(|| line.strip_prefix("import "))
            .or_else(|| line.strip_prefix("using ")),
        LanguageFamily::Ruby => line
            .strip_prefix("require ")
            .or_else(|| line.strip_prefix("require_relative ")),
        LanguageFamily::Shell | LanguageFamily::Other => line
            .strip_prefix("source ")
            .or_else(|| line.strip_prefix(". ")),
    }?;
    let cleaned = candidate
        .trim()
        .trim_matches(';')
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches('<')
        .trim_matches('>')
        .trim_start_matches("crate::")
        .trim_start_matches("self::")
        .trim_start_matches("./")
        .trim_start_matches("../")
        .to_string();
    (!cleaned.is_empty()).then_some(cleaned)
}

fn extract_calls(line: &str, family: LanguageFamily) -> Vec<String> {
    let mut calls = Vec::new();
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut quote: Option<u8> = None;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active_quote) = quote {
            if byte == b'\\' {
                index += 2;
                continue;
            }
            if byte == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        if byte == b'"' || byte == b'\'' || byte == b'`' {
            quote = Some(byte);
            index += 1;
            continue;
        }
        if byte == b'(' {
            let mut end = index;
            while end > 0 && bytes[end - 1].is_ascii_whitespace() {
                end -= 1;
            }
            let mut start = end;
            while start > 0 {
                let candidate = bytes[start - 1];
                if candidate.is_ascii_alphanumeric() || candidate == b'_' {
                    start -= 1;
                } else {
                    break;
                }
            }
            if start < end {
                let name = &line[start..end];
                if is_identifier(name) && !is_declaration_name(line, name, family) {
                    calls.push(name.to_string());
                }
            }
        }
        index += 1;
    }
    calls.sort();
    calls.dedup();
    calls
}

fn is_declaration_name(line: &str, name: &str, family: LanguageFamily) -> bool {
    parse_declaration(line, family).is_some_and(|(declared, _, _)| declared == name)
}

fn current_owner_id(
    scopes: &[(u64, usize, i64)],
    python_indents: &[(u64, usize)],
    family: LanguageFamily,
) -> Option<u64> {
    if matches!(family, LanguageFamily::Python) {
        python_indents.last().map(|(id, _)| *id)
    } else {
        scopes.last().map(|(id, _, _)| *id)
    }
}

fn strip_visibility(line: &str) -> &str {
    let mut current = line;
    for prefix in ["pub(crate) ", "pub(super) ", "pub(self) ", "pub "] {
        if let Some(rest) = current.strip_prefix(prefix) {
            current = rest;
            break;
        }
    }
    current
}

fn take_identifier(value: &str) -> Option<String> {
    let identifier: String = value
        .chars()
        .take_while(|character| character.is_alphanumeric() || *character == '_' || *character == '$')
        .collect();
    is_identifier(&identifier).then_some(identifier)
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_alphabetic() || first == '_' || first == '$')
        && chars.all(|character| character.is_alphanumeric() || character == '_' || character == '$')
}

fn compact_signature(line: &str) -> String {
    let mut output = String::new();
    let mut previous_space = false;
    for character in line.trim().chars() {
        if character.is_whitespace() {
            if !previous_space {
                output.push(' ');
                previous_space = true;
            }
        } else {
            output.push(character);
            previous_space = false;
        }
        if output.len() >= 500 {
            output.push('…');
            break;
        }
    }
    output
}

fn is_doc_line(line: &str, family: LanguageFamily) -> bool {
    match family {
        LanguageFamily::Rust => line.starts_with("///") || line.starts_with("//!") || line.starts_with("/**"),
        LanguageFamily::Python | LanguageFamily::Shell | LanguageFamily::Ruby => line.starts_with('#'),
        _ => line.starts_with("///") || line.starts_with("/**") || line.starts_with("//"),
    }
}

fn clean_doc_line(line: &str, family: LanguageFamily) -> String {
    match family {
        LanguageFamily::Rust => line
            .trim_start_matches("///")
            .trim_start_matches("//!")
            .trim_start_matches("/**")
            .trim_end_matches("*/")
            .trim()
            .to_string(),
        LanguageFamily::Python | LanguageFamily::Shell | LanguageFamily::Ruby => {
            line.trim_start_matches('#').trim().to_string()
        }
        _ => line
            .trim_start_matches("///")
            .trim_start_matches("//")
            .trim_start_matches("/**")
            .trim_end_matches("*/")
            .trim()
            .to_string(),
    }
}

fn opens_scope(line: &str) -> bool {
    line.contains('{') || line.ends_with(':') || line == "do"
}

fn starts_with_control_keyword(line: &str) -> bool {
    ["if ", "for ", "while ", "switch ", "catch ", "return ", "throw ", "new "]
        .iter()
        .any(|prefix| line.starts_with(prefix))
}

fn is_call_keyword(name: &str) -> bool {
    matches!(
        name,
        "if" | "for" | "while" | "match" | "switch" | "catch" | "return" | "sizeof" | "typeof"
            | "fn" | "function" | "def" | "class" | "struct" | "enum" | "trait" | "interface"
            | "Some" | "Ok" | "Err" | "println" | "print" | "assert" | "assert_eq" | "vec"
    )
}

fn estimate_line_complexity(line: &str) -> u32 {
    let lower = line.to_ascii_lowercase();
    let mut score = 0_u32;
    for marker in [" if ", "if(", " if(", " else ", " match ", " for ", " while ", " case ", "&&", "||", "? "] {
        score = score.saturating_add(lower.matches(marker).count() as u32);
    }
    score
}

fn count_char_outside_strings(line: &str, target: char) -> usize {
    let mut count = 0;
    let mut quote = None;
    let mut escaped = false;
    for character in line.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if character == active {
                quote = None;
            }
            continue;
        }
        if matches!(character, '"' | '\'' | '`') {
            quote = Some(character);
        } else if character == target {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parses_rust_symbols_and_calls() {
        let source = "pub fn alpha() { beta(); }\nfn beta() {}\n";
        let parsed = parse_source("src/lib.rs", Path::new("src/lib.rs"), source, 1)
            .unwrap_or_else(|| panic!("Rust parser should be selected"));
        assert_eq!(parsed.symbols.len(), 2);
        assert!(parsed.unresolved_calls.iter().any(|(_, call)| call == "beta"));
    }

    #[test]
    fn detects_python_class_and_method() {
        let source = "class User:\n    def login(self):\n        check()\n\ndef top_level():\n    return True\n";
        let parsed = parse_source("app.py", Path::new("app.py"), source, 1)
            .unwrap_or_else(|| panic!("Python parser should be selected"));
        assert_eq!(parsed.symbols.len(), 3);
        assert_eq!(parsed.symbols[0].kind, SymbolKind::Class);
        assert_eq!(parsed.symbols[1].qualified_name, "User::login");
        assert_eq!(parsed.symbols[2].qualified_name, "top_level");
    }

    #[test]
    fn classifies_rust_test_attributes() {
        let source = "/// verifies authentication\n#[test]\nfn login_test() {}\n";
        let parsed = parse_source("src/lib.rs", Path::new("src/lib.rs"), source, 1)
            .unwrap_or_else(|| panic!("Rust parser should be selected"));
        assert_eq!(parsed.symbols.len(), 1);
        assert_eq!(parsed.symbols[0].kind, SymbolKind::Test);
        assert_eq!(parsed.symbols[0].doc, "verifies authentication");
    }
}
