use crate::model::{GraphIndex, SearchHit, SymbolKind};
use crate::util::tokenize;
use std::collections::{BTreeMap, BTreeSet};

pub fn find_symbols(
    graph: &GraphIndex,
    query: &str,
    kind: Option<SymbolKind>,
    limit: usize,
) -> Vec<SearchHit> {
    let query_lower = query.to_ascii_lowercase();
    let query_tokens = tokenize(query);
    let mut scores: BTreeMap<u64, (i64, BTreeSet<String>)> = BTreeMap::new();

    for symbol in graph.symbols.values() {
        if kind.is_some_and(|expected| symbol.kind != expected) {
            continue;
        }
        let name = symbol.name.to_ascii_lowercase();
        let qualified = symbol.qualified_name.to_ascii_lowercase();
        let signature = symbol.signature.to_ascii_lowercase();
        let path = graph
            .file_for_symbol(symbol)
            .map_or("", |file| file.path.as_str())
            .to_ascii_lowercase();
        let mut score = 0_i64;
        let mut reasons = BTreeSet::new();

        if name == query_lower {
            score += 10_000;
            reasons.insert("exact-name".to_string());
        } else if name.starts_with(&query_lower) {
            score += 6_000;
            reasons.insert("name-prefix".to_string());
        } else if name.contains(&query_lower) {
            score += 4_000;
            reasons.insert("name-substring".to_string());
        }
        if qualified.contains(&query_lower) {
            score += 2_500;
            reasons.insert("qualified-name".to_string());
        }
        if path.contains(&query_lower) {
            score += 2_000;
            reasons.insert("path".to_string());
        }
        if signature.contains(&query_lower) {
            score += 1_500;
            reasons.insert("signature".to_string());
        }

        let haystack_tokens: BTreeSet<String> = tokenize(&format!(
            "{} {} {} {} {}",
            symbol.name, symbol.qualified_name, symbol.signature, symbol.doc, path
        ))
        .into_iter()
        .collect();
        for token in &query_tokens {
            if haystack_tokens.contains(token) {
                score += 700;
                reasons.insert(format!("token:{token}"));
            }
        }

        if score > 0 {
            scores.insert(symbol.id, (score, reasons));
        }
    }

    let seed_ids: Vec<u64> = scores.keys().copied().collect();
    for seed_id in seed_ids {
        let base = scores.get(&seed_id).map_or(0, |entry| entry.0);
        for edge in graph.outgoing(seed_id).chain(graph.incoming(seed_id)) {
            let neighbor = if edge.from == seed_id { edge.to } else { edge.from };
            if graph.symbols.contains_key(&neighbor) {
                let boost = (base / 8).max(150) * i64::from(edge.confidence_milli) / 1000;
                let entry = scores.entry(neighbor).or_default();
                entry.0 += boost;
                entry.1.insert(format!("graph:{}", edge.kind.as_str()));
            }
        }
    }

    let mut hits: Vec<SearchHit> = scores
        .into_iter()
        .map(|(symbol_id, (score_milli, reasons))| SearchHit {
            symbol_id,
            score_milli,
            reasons: reasons.into_iter().collect(),
        })
        .collect();
    hits.sort_by(|left, right| {
        right
            .score_milli
            .cmp(&left.score_milli)
            .then_with(|| left.symbol_id.cmp(&right.symbol_id))
    });
    hits.truncate(limit);
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FileRecord, GraphIndex, Symbol};

    #[test]
    fn exact_name_wins() {
        let mut graph = GraphIndex::empty(".".to_string(), 0);
        graph.insert_file(
            FileRecord {
                id: 1,
                path: "auth.rs".to_string(),
                language: "rust".to_string(),
                hash: 1,
                bytes: 1,
                modified_unix_ms: 0,
                line_count: 1,
            },
            vec![Symbol {
                id: 2,
                file_id: 1,
                name: "login".to_string(),
                qualified_name: "login".to_string(),
                kind: SymbolKind::Function,
                line_start: 1,
                line_end: 1,
                signature: "fn login()".to_string(),
                doc: String::new(),
                complexity: 0,
            }],
            vec![],
        );
        let hits = find_symbols(&graph, "login", None, 10);
        assert_eq!(hits[0].symbol_id, 2);
    }
}
