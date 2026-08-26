use crate::model::{ContextCellItem, GraphIndex};
use crate::secret::redact_secrets;
use crate::util::{estimate_tokens, stable_hash, tokenize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};

const START_MARKER: &str = "<!-- cse:cell";
const END_MARKER: &str = "<!-- /cse:cell -->";
const MAX_CELL_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone)]
struct ContextCell {
    id: String,
    title: String,
    kind: String,
    status: String,
    priority: u8,
    always: bool,
    path: String,
    line_start: usize,
    line_end: usize,
    tags: Vec<String>,
    evidence_paths: Vec<String>,
    symbols: Vec<String>,
    body: String,
}

#[derive(Debug, Clone)]
pub(crate) struct RankedCell {
    pub item: ContextCellItem,
    pub estimated_tokens: usize,
    pub source_bytes: usize,
}

pub(crate) fn load_ranked_cells(
    root: &Path,
    graph: &GraphIndex,
    query: &str,
    redact: bool,
) -> (Vec<RankedCell>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut cells = Vec::new();

    for file in graph
        .files
        .values()
        .filter(|file| file.language == "markdown")
    {
        let absolute = root.join(&file.path);
        let source = match fs::read_to_string(&absolute) {
            Ok(source) => source,
            Err(error) => {
                warnings.push(format!(
                    "cannot read context cell source {}: {error}",
                    file.path
                ));
                continue;
            }
        };
        let (mut parsed, mut parse_warnings) = parse_cells(&file.path, &source);
        cells.append(&mut parsed);
        warnings.append(&mut parse_warnings);
    }

    let mut unique = BTreeMap::new();
    for cell in cells {
        if unique.contains_key(&cell.id) {
            warnings.push(format!(
                "duplicate context cell id `{}`; keeping the first deterministic occurrence",
                cell.id
            ));
            continue;
        }
        unique.insert(cell.id.clone(), cell);
    }

    let query_tokens: BTreeSet<String> = tokenize(query).into_iter().collect();
    let query_lower = query.to_ascii_lowercase();
    let mut ranked = Vec::new();
    for cell in unique.into_values() {
        if matches!(cell.status.as_str(), "deprecated" | "archived" | "rejected") {
            continue;
        }
        let score = score_cell(&cell, &query_lower, &query_tokens);
        if score <= 0 && !cell.always {
            continue;
        }
        let missing_evidence: Vec<String> = cell
            .evidence_paths
            .iter()
            .filter(|path| !root.join(path).is_file())
            .cloned()
            .collect();
        if !missing_evidence.is_empty() {
            warnings.push(format!(
                "context cell `{}` references missing evidence: {}",
                cell.id,
                missing_evidence.join(", ")
            ));
        }
        if cell.status == "draft" {
            warnings.push(format!(
                "context cell `{}` is draft evidence; verify it before high-risk changes",
                cell.id
            ));
        }
        let redacted = if redact {
            redact_secrets(&cell.body)
        } else {
            crate::secret::RedactionResult {
                content: cell.body,
                redactions: 0,
            }
        };
        let content_hash = format!("{:016x}", stable_hash(redacted.content.as_bytes()));
        let estimated_tokens = estimate_tokens(&redacted.content).saturating_add(40);
        ranked.push(RankedCell {
            source_bytes: redacted.content.len(),
            estimated_tokens,
            item: ContextCellItem {
                id: cell.id,
                title: cell.title,
                kind: cell.kind,
                status: cell.status,
                priority: cell.priority,
                path: cell.path,
                line_start: cell.line_start,
                line_end: cell.line_end,
                score_milli: score,
                tags: cell.tags,
                evidence_paths: cell.evidence_paths,
                symbols: cell.symbols,
                content: redacted.content,
                content_hash,
                redactions: redacted.redactions,
            },
        });
    }
    ranked.sort_by(|left, right| {
        right
            .item
            .score_milli
            .cmp(&left.item.score_milli)
            .then_with(|| right.item.priority.cmp(&left.item.priority))
            .then_with(|| left.item.id.cmp(&right.item.id))
    });
    warnings.sort();
    warnings.dedup();
    (ranked, warnings)
}

fn parse_cells(path: &str, source: &str) -> (Vec<ContextCell>, Vec<String>) {
    let mut cells = Vec::new();
    let mut warnings = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    let mut cursor = 0;
    let mut fence: Option<String> = None;
    while cursor < lines.len() {
        let trimmed = lines[cursor].trim_start();
        if let Some(active) = fence.as_deref() {
            if trimmed.starts_with(active) {
                fence = None;
            }
            cursor += 1;
            continue;
        }
        if trimmed.starts_with("```") {
            fence = Some("```".to_string());
            cursor += 1;
            continue;
        }
        if trimmed.starts_with("~~~") {
            fence = Some("~~~".to_string());
            cursor += 1;
            continue;
        }
        if !lines[cursor].trim_start().starts_with(START_MARKER) {
            cursor += 1;
            continue;
        }
        let marker_line = cursor + 1;
        let mut header = String::new();
        let mut header_cursor = cursor;
        let first = lines[cursor].trim();
        if let Some(rest) = first.strip_prefix(START_MARKER) {
            if let Some((inline, _)) = rest.split_once("-->") {
                header.push_str(inline.trim());
            } else {
                header.push_str(rest.trim());
                header_cursor += 1;
                while header_cursor < lines.len() {
                    if let Some((before, _)) = lines[header_cursor].split_once("-->") {
                        if !header.is_empty() {
                            header.push('\n');
                        }
                        header.push_str(before.trim());
                        break;
                    }
                    if !header.is_empty() {
                        header.push('\n');
                    }
                    header.push_str(lines[header_cursor].trim());
                    header_cursor += 1;
                }
            }
        }
        if header_cursor >= lines.len() {
            warnings.push(format!(
                "{path}:{marker_line}: unclosed context cell header"
            ));
            break;
        }
        let body_start = header_cursor + 1;
        let Some(relative_end) = lines[body_start..]
            .iter()
            .position(|line| line.trim() == END_MARKER)
        else {
            warnings.push(format!(
                "{path}:{marker_line}: context cell is missing `{END_MARKER}`"
            ));
            break;
        };
        let body_end = body_start + relative_end;
        let body = lines[body_start..body_end].join("\n").trim().to_string();
        let metadata = parse_header(&header);
        let id = metadata.get("id").cloned().unwrap_or_default();
        if !valid_id(&id) {
            warnings.push(format!(
                "{path}:{marker_line}: context cell id must use letters, numbers, `.`, `_`, or `-`"
            ));
            cursor = body_end + 1;
            continue;
        }
        if body.is_empty() {
            warnings.push(format!(
                "{path}:{marker_line}: context cell `{id}` is empty"
            ));
            cursor = body_end + 1;
            continue;
        }
        if body.len() > MAX_CELL_BYTES {
            warnings.push(format!(
                "{path}:{marker_line}: context cell `{id}` exceeds {MAX_CELL_BYTES} bytes"
            ));
            cursor = body_end + 1;
            continue;
        }
        let raw_evidence = csv(metadata.get("paths").or_else(|| metadata.get("evidence")));
        let mut evidence_paths = Vec::new();
        for evidence in raw_evidence {
            if safe_relative_path(&evidence) {
                evidence_paths.push(evidence.replace('\\', "/"));
            } else {
                warnings.push(format!(
                    "{path}:{marker_line}: ignored unsafe evidence path `{evidence}`"
                ));
            }
        }
        let priority = parse_priority(metadata.get("priority").map(String::as_str));
        let status = metadata
            .get("status")
            .map_or("draft", String::as_str)
            .to_ascii_lowercase();
        let title = metadata
            .get("title")
            .cloned()
            .or_else(|| first_heading(&body))
            .unwrap_or_else(|| id.clone());
        cells.push(ContextCell {
            id,
            title,
            kind: metadata
                .get("kind")
                .map_or("knowledge", String::as_str)
                .to_ascii_lowercase(),
            status,
            priority,
            always: metadata
                .get("always")
                .is_some_and(|value| matches!(value.as_str(), "true" | "yes" | "1")),
            path: path.to_string(),
            line_start: body_start + 1,
            line_end: body_end.max(body_start + 1),
            tags: csv(metadata.get("tags")),
            evidence_paths,
            symbols: csv(metadata.get("symbols")),
            body,
        });
        cursor = body_end + 1;
    }
    (cells, warnings)
}

fn parse_header(header: &str) -> BTreeMap<String, String> {
    let mut metadata = BTreeMap::new();
    for line in header.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();
        let pair = trimmed.split_once(':').or_else(|| trimmed.split_once('='));
        if let Some((key, value)) = pair {
            metadata.insert(
                key.trim().to_ascii_lowercase(),
                value.trim().trim_matches('"').to_string(),
            );
        }
    }
    metadata
}

fn score_cell(cell: &ContextCell, query: &str, query_tokens: &BTreeSet<String>) -> i64 {
    let identity = tokenize(&format!("{} {}", cell.id, cell.title));
    let tags = tokenize(&cell.tags.join(" "));
    let evidence = tokenize(&cell.evidence_paths.join(" "));
    let symbols = tokenize(&cell.symbols.join(" "));
    let body = tokenize(&cell.body);
    let mut score = i64::from(cell.priority) * 2;
    score += overlap(query_tokens, &identity) * 180;
    score += overlap(query_tokens, &symbols) * 170;
    score += overlap(query_tokens, &tags) * 130;
    score += overlap(query_tokens, &evidence) * 100;
    score += overlap(query_tokens, &body) * 35;
    let title_lower = cell.title.to_ascii_lowercase();
    if !query.is_empty()
        && (title_lower.contains(query) || cell.body.to_ascii_lowercase().contains(query))
    {
        score += 300;
    }
    if cell.status == "accepted" {
        score += 80;
    }
    if cell.always {
        score += 500;
    }
    score
}

fn overlap(query: &BTreeSet<String>, values: &[String]) -> i64 {
    values.iter().filter(|value| query.contains(*value)).count() as i64
}

fn csv(value: Option<&String>) -> Vec<String> {
    value
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

fn safe_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !value.is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn parse_priority(value: Option<&str>) -> u8 {
    match value.unwrap_or("normal").to_ascii_lowercase().as_str() {
        "critical" => 100,
        "high" => 80,
        "normal" | "medium" => 50,
        "low" => 20,
        numeric => numeric.parse::<u8>().unwrap_or(50).min(100),
    }
}

fn first_heading(body: &str) -> Option<String> {
    body.lines().find_map(|line| {
        line.trim()
            .strip_prefix('#')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiline_markdown_cell() {
        let source = r#"# Project context

<!-- cse:cell
id: auth.local-session
title: Local session boundary
kind: constraint
status: accepted
priority: critical
tags: auth, security
paths: src/server.rs, dashboard/src/main.ts
symbols: check_authorization, ApiClient
-->
The dashboard is local-only and uses an HttpOnly session cookie.
<!-- /cse:cell -->
"#;
        let (cells, warnings) = parse_cells("CODESPACE.md", source);
        assert!(warnings.is_empty());
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].id, "auth.local-session");
        assert_eq!(cells[0].priority, 100);
        assert_eq!(cells[0].evidence_paths.len(), 2);
        assert_eq!(cells[0].line_start, 13);
    }

    #[test]
    fn rejects_path_escape() {
        let source = "<!-- cse:cell\nid: unsafe\npaths: ../secret\n-->\n# Body\n<!-- /cse:cell -->";
        let (cells, warnings) = parse_cells("CODESPACE.md", source);
        assert_eq!(cells.len(), 1);
        assert!(cells[0].evidence_paths.is_empty());
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("unsafe evidence"))
        );
    }

    #[test]
    fn ignores_protocol_examples_inside_fences() {
        let source = r#"# Authoring guide

```markdown
<!-- cse:cell
id: example.only
-->
This is documentation, not a live cell.
<!-- /cse:cell -->
```
"#;
        let (cells, warnings) = parse_cells("GUIDE.md", source);
        assert!(cells.is_empty());
        assert!(warnings.is_empty());
    }
}
