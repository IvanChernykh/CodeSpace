use crate::model::{ContextBundle, ContextItem, GraphIndex, Result};
use crate::search::find_symbols;
use crate::secret::redact_secrets;
use crate::util::{estimate_tokens, now_unix_ms};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ContextOptions {
    pub max_tokens: usize,
    pub max_items: usize,
    pub include_docs: bool,
    pub redact_secrets: bool,
    pub neighbor_lines: usize,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            max_tokens: 1_200,
            max_items: 8,
            include_docs: true,
            redact_secrets: true,
            neighbor_lines: 2,
        }
    }
}

pub fn build_context(
    root: &Path,
    graph: &GraphIndex,
    query: &str,
    options: &ContextOptions,
) -> Result<ContextBundle> {
    let hits = find_symbols(graph, query, None, options.max_items.saturating_mul(4).max(16));
    let mut items = Vec::new();
    let mut warnings = Vec::new();
    let mut source_bytes = 0_usize;
    let mut returned_bytes = 0_usize;
    let mut used_tokens = 0_usize;
    let mut seen = BTreeSet::new();
    let mut source_files_counted = BTreeSet::new();
    let mut selected_per_file: BTreeMap<String, usize> = BTreeMap::new();

    if options.max_tokens <= 24 {
        warnings.push("token budget must exceed the per-item metadata overhead of 24 tokens".to_string());
        return Ok(ContextBundle {
            query: query.to_string(),
            generated_unix_ms: now_unix_ms(),
            estimated_tokens: 0,
            source_bytes: 0,
            returned_bytes: 0,
            items,
            warnings,
        });
    }

    for hit in hits {
        if items.len() >= options.max_items {
            break;
        }
        let Some(symbol) = graph.symbols.get(&hit.symbol_id) else {
            continue;
        };
        let Some(file) = graph.file_for_symbol(symbol) else {
            continue;
        };
        if !seen.insert((file.path.clone(), symbol.line_start, symbol.line_end)) {
            continue;
        }
        let count_for_file = selected_per_file.entry(file.path.clone()).or_default();
        if *count_for_file >= 3 {
            continue;
        }
        let absolute = root.join(&file.path);
        let bytes = match fs::read(&absolute) {
            Ok(bytes) => bytes,
            Err(error) => {
                warnings.push(format!("cannot read {}: {error}", file.path));
                continue;
            }
        };
        let source = String::from_utf8_lossy(&bytes);
        if source_files_counted.insert(file.path.clone()) {
            source_bytes = source_bytes.saturating_add(bytes.len());
        }
        let lines: Vec<&str> = source.lines().collect();
        let start = symbol.line_start.saturating_sub(options.neighbor_lines).max(1);
        let end = symbol
            .line_end
            .saturating_add(options.neighbor_lines)
            .min(lines.len().max(1));
        let mut content = compact_lines(&lines, start, end, &file.language);
        if options.include_docs && !symbol.doc.is_empty() {
            content = format!("// decision-relevant documentation: {}\n{content}", symbol.doc.replace('\n', " "));
        }
        let redacted = if options.redact_secrets {
            redact_secrets(&content)
        } else {
            crate::secret::RedactionResult {
                content,
                redactions: 0,
            }
        };
        let item_tokens = estimate_tokens(&redacted.content).saturating_add(24);
        if used_tokens.saturating_add(item_tokens) > options.max_tokens {
            if items.is_empty() {
                let shortened = truncate_to_token_budget(&redacted.content, options.max_tokens.saturating_sub(24));
                returned_bytes = returned_bytes.saturating_add(shortened.len());
                used_tokens = used_tokens.saturating_add(estimate_tokens(&shortened) + 24);
                items.push(ContextItem {
                    path: file.path.clone(),
                    language: file.language.clone(),
                    symbol: symbol.qualified_name.clone(),
                    kind: symbol.kind,
                    line_start: start,
                    line_end: end,
                    score_milli: hit.score_milli,
                    content: shortened,
                    redactions: redacted.redactions,
                });
            }
            break;
        }
        returned_bytes = returned_bytes.saturating_add(redacted.content.len());
        used_tokens = used_tokens.saturating_add(item_tokens);
        *count_for_file += 1;
        items.push(ContextItem {
            path: file.path.clone(),
            language: file.language.clone(),
            symbol: symbol.qualified_name.clone(),
            kind: symbol.kind,
            line_start: start,
            line_end: end,
            score_milli: hit.score_milli,
            content: redacted.content,
            redactions: redacted.redactions,
        });
    }

    if items.is_empty() {
        warnings.push("no matching symbols found; try a symbol name, file path, or architectural term".to_string());
    }
    let total_redactions: usize = items.iter().map(|item| item.redactions).sum();
    if total_redactions > 0 {
        warnings.push(format!("redacted {total_redactions} potential secret(s)"));
    }
    Ok(ContextBundle {
        query: query.to_string(),
        generated_unix_ms: now_unix_ms(),
        estimated_tokens: used_tokens,
        source_bytes,
        returned_bytes,
        items,
        warnings,
    })
}

fn compact_lines(lines: &[&str], start: usize, end: usize, language: &str) -> String {
    let mut output = String::new();
    let mut blank_pending = false;
    let mut in_block_comment = false;
    for line_number in start..=end {
        let Some(line) = lines.get(line_number.saturating_sub(1)) else {
            continue;
        };
        let trimmed = line.trim();
        if trimmed.starts_with("/*") {
            in_block_comment = !trimmed.contains("*/");
            continue;
        }
        if in_block_comment {
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }
        if is_nonsemantic_comment(trimmed, language) {
            continue;
        }
        if trimmed.is_empty() {
            blank_pending = true;
            continue;
        }
        if blank_pending && !output.is_empty() {
            output.push('\n');
        }
        blank_pending = false;
        output.push_str(&format!("{line_number:>5} | {}\n", collapse_whitespace(line)));
    }
    output.trim_end().to_string()
}

fn is_nonsemantic_comment(line: &str, language: &str) -> bool {
    if line.starts_with("///") || line.starts_with("//!") || line.starts_with("/**") {
        return false;
    }
    match language {
        "python" | "ruby" | "shell" => line.starts_with('#') && !line.starts_with("#!"),
        "markdown" => false,
        _ => line.starts_with("//"),
    }
}

fn collapse_whitespace(line: &str) -> String {
    let indentation = line.chars().take_while(|character| character.is_whitespace()).count().min(8);
    let body = line.trim();
    let mut output = " ".repeat(indentation);
    let mut previous_space = false;
    for character in body.chars() {
        if character.is_whitespace() {
            if !previous_space {
                output.push(' ');
                previous_space = true;
            }
        } else {
            output.push(character);
            previous_space = false;
        }
    }
    output
}

fn truncate_to_token_budget(value: &str, budget: usize) -> String {
    let max_chars = budget.saturating_mul(4);
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut output: String = value.chars().take(max_chars.saturating_sub(1)).collect();
    output.push('…');
    output
}

pub fn render_markdown(bundle: &ContextBundle) -> String {
    let mut output = format!(
        "# CodeSpace context\n\nQuery: `{}`  \nEstimated tokens: **{}**  \nItems: **{}**\n\n",
        bundle.query,
        bundle.estimated_tokens,
        bundle.items.len()
    );
    for item in &bundle.items {
        output.push_str(&format!(
            "## `{}` — `{}` ({}:{})\n\n```{}\n{}\n```\n\n",
            item.symbol,
            item.kind.as_str(),
            item.path,
            item.line_start,
            item.language,
            item.content
        ));
    }
    if !bundle.warnings.is_empty() {
        output.push_str("## Warnings\n\n");
        for warning in &bundle.warnings {
            output.push_str(&format!("- {warning}\n"));
        }
    }
    output
}

pub fn render_plain(bundle: &ContextBundle) -> String {
    let mut output = format!(
        "QUERY: {}\nTOKENS_ESTIMATE: {}\nITEMS: {}\n\n",
        bundle.query,
        bundle.estimated_tokens,
        bundle.items.len()
    );
    for item in &bundle.items {
        output.push_str(&format!(
            "--- {} [{}] {}:{}-{} score={} ---\n{}\n\n",
            item.symbol,
            item.kind.as_str(),
            item.path,
            item.line_start,
            item.line_end,
            item.score_milli,
            item.content
        ));
    }
    for warning in &bundle.warnings {
        output.push_str(&format!("WARNING: {warning}\n"));
    }
    output
}

pub fn render_json(bundle: &ContextBundle) -> String {
    let mut output = format!(
        "{{\"query\":\"{}\",\"generated_unix_ms\":{},\"estimated_tokens\":{},\"source_bytes\":{},\"returned_bytes\":{},\"items\":[",
        crate::util::json_escape(&bundle.query),
        bundle.generated_unix_ms,
        bundle.estimated_tokens,
        bundle.source_bytes,
        bundle.returned_bytes
    );
    for (index, item) in bundle.items.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"path\":\"{}\",\"language\":\"{}\",\"symbol\":\"{}\",\"kind\":\"{}\",\"line_start\":{},\"line_end\":{},\"score_milli\":{},\"redactions\":{},\"content\":\"{}\"}}",
            crate::util::json_escape(&item.path),
            crate::util::json_escape(&item.language),
            crate::util::json_escape(&item.symbol),
            item.kind.as_str(),
            item.line_start,
            item.line_end,
            item.score_milli,
            item.redactions,
            crate::util::json_escape(&item.content)
        ));
    }
    output.push_str("],\"warnings\":[");
    for (index, warning) in bundle.warnings.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!("\"{}\"", crate::util::json_escape(warning)));
    }
    output.push_str("]}");
    output
}
