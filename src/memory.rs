use crate::model::{Decision, GraphIndex};
use crate::util::{now_unix_ms, stable_id};

#[derive(Debug, Clone)]
pub struct RememberInput {
    pub file: String,
    pub symbol: String,
    pub session: String,
    pub agent: String,
    pub summary: String,
    pub rationale: String,
    pub tags: Vec<String>,
}

pub fn remember(graph: &mut GraphIndex, input: RememberInput) -> u64 {
    let timestamp = now_unix_ms();
    let id = stable_id(&[
        "decision",
        &timestamp.to_string(),
        &input.file,
        &input.symbol,
        &input.session,
        &input.summary,
    ]);
    graph.decisions.insert(
        id,
        Decision {
            id,
            timestamp_unix_ms: timestamp,
            file: input.file,
            symbol: input.symbol,
            session: input.session,
            agent: input.agent,
            summary: input.summary,
            rationale: input.rationale,
            tags: input.tags,
        },
    );
    graph.updated_unix_ms = timestamp;
    id
}

pub fn history<'a>(graph: &'a GraphIndex, target: &str, limit: usize) -> Vec<&'a Decision> {
    let target_lower = target.to_ascii_lowercase();
    let mut decisions: Vec<&Decision> = graph
        .decisions
        .values()
        .filter(|decision| {
            target.is_empty()
                || decision.file.to_ascii_lowercase().contains(&target_lower)
                || decision.symbol.to_ascii_lowercase().contains(&target_lower)
                || decision.summary.to_ascii_lowercase().contains(&target_lower)
                || decision.tags.iter().any(|tag| tag.to_ascii_lowercase() == target_lower)
        })
        .collect();
    decisions.sort_by(|left, right| right.timestamp_unix_ms.cmp(&left.timestamp_unix_ms));
    decisions.truncate(limit);
    decisions
}

pub fn render_history_plain(decisions: &[&Decision]) -> String {
    if decisions.is_empty() {
        return "No decisions found.\n".to_string();
    }
    let mut output = String::new();
    for decision in decisions {
        output.push_str(&format!(
            "{}  {}  {}\n  {}\n  rationale: {}\n  session={} agent={} tags={}\n\n",
            decision.timestamp_unix_ms,
            decision.file,
            decision.symbol,
            decision.summary,
            decision.rationale,
            decision.session,
            decision.agent,
            decision.tags.join(",")
        ));
    }
    output
}

pub fn render_history_json(decisions: &[&Decision]) -> String {
    let mut output = String::from("[");
    for (index, decision) in decisions.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"id\":{},\"timestamp_unix_ms\":{},\"file\":\"{}\",\"symbol\":\"{}\",\"session\":\"{}\",\"agent\":\"{}\",\"summary\":\"{}\",\"rationale\":\"{}\",\"tags\":[{}]}}",
            decision.id,
            decision.timestamp_unix_ms,
            crate::util::json_escape(&decision.file),
            crate::util::json_escape(&decision.symbol),
            crate::util::json_escape(&decision.session),
            crate::util::json_escape(&decision.agent),
            crate::util::json_escape(&decision.summary),
            crate::util::json_escape(&decision.rationale),
            decision
                .tags
                .iter()
                .map(|tag| format!("\"{}\"", crate::util::json_escape(tag)))
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    output.push(']');
    output
}
