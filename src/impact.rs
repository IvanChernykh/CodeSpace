use crate::model::{EdgeKind, Error, GraphIndex, ImpactNode, ImpactReport, Result, SymbolKind};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::process::Command;

pub fn analyze(
    root: &Path,
    graph: &GraphIndex,
    from_ref: &str,
    to_ref: &str,
    depth_limit: usize,
) -> Result<ImpactReport> {
    let changed = changed_lines(root, from_ref, to_ref)?;
    let changed_files: Vec<String> = changed.keys().cloned().collect();
    let mut changed_symbols = Vec::new();
    let mut changed_ids = BTreeSet::new();

    for (path, lines) in &changed {
        let Some(file_id) = graph.files_by_path.get(path) else {
            changed_symbols.push(ImpactNode {
                symbol_id: 0,
                path: path.clone(),
                symbol: "<file unavailable in current index>".to_string(),
                kind: SymbolKind::Unknown,
                depth: 0,
                reason: "file was deleted, renamed, ignored, or is not a supported source type".to_string(),
            });
            continue;
        };
        let mut matched = false;
        for symbol in graph.symbols.values().filter(|symbol| symbol.file_id == *file_id) {
            if lines.is_empty()
                || lines
                    .iter()
                    .any(|line| *line >= symbol.line_start && *line <= symbol.line_end)
            {
                matched = true;
                changed_ids.insert(symbol.id);
                changed_symbols.push(ImpactNode {
                    symbol_id: symbol.id,
                    path: path.clone(),
                    symbol: symbol.qualified_name.clone(),
                    kind: symbol.kind,
                    depth: 0,
                    reason: "changed line intersects symbol".to_string(),
                });
            }
        }
        if !matched {
            for symbol in graph.symbols.values().filter(|symbol| symbol.file_id == *file_id) {
                changed_ids.insert(symbol.id);
                changed_symbols.push(ImpactNode {
                    symbol_id: symbol.id,
                    path: path.clone(),
                    symbol: symbol.qualified_name.clone(),
                    kind: symbol.kind,
                    depth: 0,
                    reason: "file changed outside indexed symbol boundaries".to_string(),
                });
            }
        }
    }

    let mut queue: VecDeque<(u64, usize)> = changed_ids.iter().map(|id| (*id, 0)).collect();
    let mut visited = changed_ids.clone();
    let mut affected = Vec::new();
    while let Some((current, depth)) = queue.pop_front() {
        if depth >= depth_limit {
            continue;
        }
        for edge in graph.incoming(current) {
            if !matches!(edge.kind, EdgeKind::Calls | EdgeKind::References | EdgeKind::Imports | EdgeKind::Contains) {
                continue;
            }
            let candidate = edge.from;
            if !graph.symbols.contains_key(&candidate) || !visited.insert(candidate) {
                continue;
            }
            let Some(symbol) = graph.symbols.get(&candidate) else {
                continue;
            };
            let Some(file) = graph.file_for_symbol(symbol) else {
                continue;
            };
            let next_depth = depth + 1;
            affected.push(ImpactNode {
                symbol_id: symbol.id,
                path: file.path.clone(),
                symbol: symbol.qualified_name.clone(),
                kind: symbol.kind,
                depth: next_depth,
                reason: format!("incoming {} edge to impacted node", edge.kind.as_str()),
            });
            queue.push_back((candidate, next_depth));
        }
    }
    affected.sort_by(|left, right| {
        left.depth
            .cmp(&right.depth)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.symbol.cmp(&right.symbol))
    });

    let risk_score = compute_risk(&changed_files, &changed_symbols, &affected);
    let mut warnings = Vec::new();
    if changed_files.iter().any(|path| {
        path.contains("auth")
            || path.contains("security")
            || path.contains("payment")
            || path.contains("migration")
            || path.contains("schema")
    }) {
        warnings.push("security-, payment-, or schema-sensitive path changed".to_string());
    }
    if changed_files.iter().any(|path| path.contains("test")) {
        warnings.push("test files changed; verify that production behavior is independently covered".to_string());
    }
    if affected.len() > 50 {
        warnings.push("wide blast radius; split the change or require targeted regression tests".to_string());
    }

    Ok(ImpactReport {
        from_ref: from_ref.to_string(),
        to_ref: to_ref.to_string(),
        changed_files,
        changed_symbols,
        affected,
        risk_score,
        warnings,
    })
}

fn changed_lines(root: &Path, from_ref: &str, to_ref: &str) -> Result<BTreeMap<String, BTreeSet<usize>>> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["diff", "--unified=0", "--no-ext-diff", from_ref, to_ref, "--"])
        .output()
        .map_err(|error| Error::Git(format!("cannot start git: {error}")))?;
    if !output.status.success() {
        return Err(Error::Git(String::from_utf8_lossy(&output.stderr).trim().to_string()));
    }
    let diff = String::from_utf8_lossy(&output.stdout);
    let mut result: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    let mut old_path = String::new();
    let mut current_path = String::new();
    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("--- a/") {
            old_path = path.to_string();
        } else if let Some(path) = line.strip_prefix("+++ b/") {
            current_path = path.to_string();
            result.entry(current_path.clone()).or_default();
        } else if line == "+++ /dev/null" {
            current_path.clone_from(&old_path);
            if !current_path.is_empty() {
                result.entry(current_path.clone()).or_default();
            }
        } else if line.starts_with("@@") && !current_path.is_empty() {
            if let Some((start, count)) = parse_new_hunk(line) {
                let lines = result.entry(current_path.clone()).or_default();
                if count == 0 {
                    lines.insert(start.max(1));
                } else {
                    for line_number in start..start.saturating_add(count) {
                        lines.insert(line_number);
                    }
                }
            }
        }
    }
    Ok(result)
}

fn parse_new_hunk(line: &str) -> Option<(usize, usize)> {
    let plus = line.split_whitespace().find(|part| part.starts_with('+'))?;
    let range = plus.trim_start_matches('+');
    let (start, count) = range.split_once(',').unwrap_or((range, "1"));
    Some((start.parse().ok()?, count.parse().ok()?))
}

fn compute_risk(changed_files: &[String], changed: &[ImpactNode], affected: &[ImpactNode]) -> u8 {
    let mut score = 5_usize;
    score += changed_files.len().saturating_mul(4);
    score += changed.len().saturating_mul(2);
    score += affected.len().min(60);
    score += affected.iter().map(|node| 4_usize.saturating_sub(node.depth)).sum::<usize>();
    if changed_files.iter().any(|path| path.contains("auth") || path.contains("security")) {
        score += 15;
    }
    if changed_files.iter().any(|path| path.contains("migration") || path.contains("schema")) {
        score += 15;
    }
    score.min(100) as u8
}

pub fn render_plain(report: &ImpactReport) -> String {
    let mut output = format!(
        "IMPACT {}..{}\nRisk score: {}/100\nChanged files: {}\nChanged symbols: {}\nAffected symbols: {}\n\n",
        report.from_ref,
        report.to_ref,
        report.risk_score,
        report.changed_files.len(),
        report.changed_symbols.len(),
        report.affected.len()
    );
    output.push_str("CHANGED\n");
    for node in &report.changed_symbols {
        output.push_str(&format!("  {}:{} [{}] {}\n", node.path, node.symbol, node.kind.as_str(), node.reason));
    }
    output.push_str("\nAFFECTED\n");
    for node in &report.affected {
        output.push_str(&format!(
            "  depth={} {}:{} [{}] {}\n",
            node.depth,
            node.path,
            node.symbol,
            node.kind.as_str(),
            node.reason
        ));
    }
    for warning in &report.warnings {
        output.push_str(&format!("WARNING: {warning}\n"));
    }
    output
}

pub fn render_json(report: &ImpactReport) -> String {
    let render_nodes = |nodes: &[ImpactNode]| {
        nodes
            .iter()
            .map(|node| {
                format!(
                    "{{\"symbol_id\":{},\"path\":\"{}\",\"symbol\":\"{}\",\"kind\":\"{}\",\"depth\":{},\"reason\":\"{}\"}}",
                    node.symbol_id,
                    crate::util::json_escape(&node.path),
                    crate::util::json_escape(&node.symbol),
                    node.kind.as_str(),
                    node.depth,
                    crate::util::json_escape(&node.reason)
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    format!(
        "{{\"from\":\"{}\",\"to\":\"{}\",\"risk_score\":{},\"changed_files\":[{}],\"changed_symbols\":[{}],\"affected\":[{}],\"warnings\":[{}]}}",
        crate::util::json_escape(&report.from_ref),
        crate::util::json_escape(&report.to_ref),
        report.risk_score,
        report.changed_files.iter().map(|path| format!("\"{}\"", crate::util::json_escape(path))).collect::<Vec<_>>().join(","),
        render_nodes(&report.changed_symbols),
        render_nodes(&report.affected),
        report.warnings.iter().map(|warning| format!("\"{}\"", crate::util::json_escape(warning))).collect::<Vec<_>>().join(",")
    )
}
