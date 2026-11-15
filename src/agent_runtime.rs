//! Shared, validated operations for CLI, MCP, and the desktop runtime.
use crate::model::{Error, Result};
use crate::routing::{BulkReadOptions, TaskKind};
use serde_json::{Value, json};
use std::path::Path;

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidArgument(message.into())
}

fn required<'a>(input: &'a Value, key: &str) -> Result<&'a str> {
    input
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| invalid(format!("non-empty `{key}` string is required")))
}

fn number(
    input: &Value,
    key: &str,
    default: usize,
    minimum: usize,
    maximum: usize,
) -> Result<usize> {
    match input.get(key) {
        None => Ok(default),
        Some(value) => value
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .filter(|v| (minimum..=maximum).contains(v))
            .ok_or_else(|| {
                invalid(format!(
                    "`{key}` must be an integer in {minimum}..={maximum}"
                ))
            }),
    }
}

pub fn execute(root: &Path, operation: &str, input: &Value) -> Result<Value> {
    let object = input
        .as_object()
        .ok_or_else(|| invalid("input must be a JSON object"))?;
    let allowed: &[&str] = match operation {
        "context-prompt" => &["query", "max_tokens", "max_items"],
        "bulk-read" => &[
            "question",
            "paths",
            "max_tokens",
            "max_snippets",
            "context_lines",
            "line_threshold",
            "task_kind",
            "start_line",
            "max_lines",
            "worker",
        ],
        "read" => &["file", "start_line", "max_lines"],
        "run-start" => &["id", "initial"],
        "run-patch" => &["id", "expected_revision", "patch"],
        "run-show" | "run-resume" => &["id"],
        "run-list" => &[],
        _ => return Err(invalid(format!("unknown runtime operation `{operation}`"))),
    };
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(invalid(format!("unknown field `{key}` for `{operation}`")));
    }
    match operation {
        "context-prompt" => {
            let graph = crate::storage::load(root)?;
            let options = crate::context::ContextOptions {
                max_tokens: number(input, "max_tokens", 1600, 128, 32000)?,
                max_items: number(input, "max_items", 10, 1, 50)?,
                ..Default::default()
            };
            let bundle =
                crate::context::build_context(root, &graph, required(input, "query")?, &options)?;
            Ok(
                json!({"pack_id": bundle.pack_id, "prompt": crate::context::render_markdown(&bundle)}),
            )
        }
        "bulk-read" => {
            let paths = input
                .get("paths")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("paths must be an array of repository-relative files"))?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| invalid("every path must be a string"))
                })
                .collect::<Result<Vec<_>>>()?;
            let task_kind = match input.get("task_kind") {
                None => TaskKind::Inspect,
                Some(v) => v
                    .as_str()
                    .and_then(TaskKind::parse)
                    .ok_or_else(|| invalid("unknown task_kind"))?,
            };
            let options = BulkReadOptions {
                max_tokens: number(input, "max_tokens", 1200, 256, 32000)?,
                max_snippets: number(input, "max_snippets", 12, 1, 50)?,
                context_lines: number(input, "context_lines", 3, 0, 20)?,
                line_threshold: number(input, "line_threshold", 350, 1, 10000)?,
                task_kind,
                start_line: input
                    .get("start_line")
                    .map(|_| number(input, "start_line", 1, 1, 1_000_000))
                    .transpose()?,
                max_lines: input
                    .get("max_lines")
                    .map(|_| number(input, "max_lines", 100, 1, 5000))
                    .transpose()?,
            };
            match input
                .get("worker")
                .and_then(Value::as_str)
                .unwrap_or("local")
            {
                "local" => Ok(crate::routing::bulk_read(
                    root,
                    required(input, "question")?,
                    &paths,
                    &options,
                )?
                .to_json()),
                "portal" => Ok(crate::portal::bulk_read(
                    root,
                    required(input, "question")?,
                    &paths,
                    &options,
                )?
                .to_json()),
                _ => Err(invalid("worker must be local or portal")),
            }
        }
        "read" => Ok(crate::routing::read_source(
            root,
            required(input, "file")?,
            number(input, "start_line", 1, 1, 1_000_000)?,
            number(input, "max_lines", 100, 1, 5000)?,
        )?
        .to_json()),
        "run-start" => crate::execution_state::start(
            root,
            required(input, "id")?,
            input
                .get("initial")
                .ok_or_else(|| invalid("initial state is required"))?,
        ),
        "run-show" => crate::execution_state::read(root, required(input, "id")?),
        "run-resume" => crate::execution_state::resume(root, required(input, "id")?),
        "run-list" => crate::execution_state::list(root),
        "run-patch" => crate::execution_state::patch(
            root,
            required(input, "id")?,
            input
                .get("expected_revision")
                .and_then(Value::as_u64)
                .ok_or_else(|| invalid("expected_revision is required"))?,
            input
                .get("patch")
                .ok_or_else(|| invalid("patch object is required"))?,
        ),
        _ => Ok(json!({})),
    }
}

pub fn parse_json(input: &str) -> Result<Value> {
    if input.len() > 65_536 {
        return Err(invalid("JSON input exceeds 64 KiB"));
    }
    serde_json::from_str(input).map_err(|error| invalid(format!("invalid JSON: {error}")))
}
