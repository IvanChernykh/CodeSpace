use crate::context::{build_context, render_json as render_context_json, render_markdown, render_plain, ContextOptions};
use crate::export;
use crate::impact;
use crate::memory::{self, RememberInput};
use crate::model::{self, Error, GraphIndex, Result, SymbolKind};
use crate::search::find_symbols;
use crate::secret::redact_secrets;
use crate::storage;
use crate::util::{json_escape, normalized_relative};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionCategory {
    Index,
    Search,
    Context,
    Impact,
    Memory,
    Export,
    System,
    Workspace,
    Skills,
    Mcp,
}

impl ActionCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Index => "index",
            Self::Search => "search",
            Self::Context => "context",
            Self::Impact => "impact",
            Self::Memory => "memory",
            Self::Export => "export",
            Self::System => "system",
            Self::Workspace => "workspace",
            Self::Skills => "skills",
            Self::Mcp => "mcp",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActionMeta {
    pub name: &'static str,
    pub description: &'static str,
    pub category: ActionCategory,
    pub read_only: bool,
    pub aliases: &'static [&'static str],
}

#[derive(Debug, Clone)]
pub struct ActionContext {
    pub root: PathBuf,
    pub graph: GraphIndex,
    pub format: OutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Plain,
    Json,
    Markdown,
}

#[derive(Debug, Clone)]
pub struct ActionResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub state_version: u64,
}

impl ActionResult {
    pub fn ok(stdout: String, state_version: u64) -> Self {
        Self {
            exit_code: 0,
            stdout,
            stderr: String::new(),
            state_version,
        }
    }

    pub fn warning(stdout: String, stderr: String, state_version: u64) -> Self {
        Self {
            exit_code: 1,
            stdout,
            stderr,
            state_version,
        }
    }

    pub fn from_error(error: Error, state_version: u64) -> Self {
        Self {
            exit_code: 2,
            stdout: String::new(),
            stderr: error.to_string(),
            state_version,
        }
    }
}

type ActionHandler = fn(&ActionContext, &ActionParams) -> Result<ActionResult>;

#[derive(Debug, Clone, Default)]
pub struct ActionParams {
    pub positional: Vec<String>,
    pub flags: BTreeMap<String, String>,
}

impl ActionParams {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.flags.get(key).map(String::as_str)
    }

    pub fn get_or(&self, key: &str, default: &str) -> String {
        self.flags.get(key).cloned().unwrap_or_else(|| default.to_string())
    }

    pub fn get_usize(&self, key: &str) -> Option<usize> {
        self.flags.get(key).and_then(|v| v.parse().ok())
    }

    pub fn get_bool(&self, key: &str) -> bool {
        self.flags.contains_key(key)
    }

    pub fn first(&self) -> Option<&str> {
        self.positional.first().map(String::as_str)
    }

    pub fn first_or(&self, default: &str) -> String {
        self.positional.first().cloned().unwrap_or_else(|| default.to_string())
    }
}

pub struct ActionRegistry {
    entries: Vec<(&'static ActionMeta, ActionHandler)>,
}

impl ActionRegistry {
    pub fn new() -> Self {
        let entries: Vec<(&'static ActionMeta, ActionHandler)> = vec![
            (&META_INIT, action_init),
            (&META_UPDATE, action_update),
            (&META_CONTEXT, action_context),
            (&META_SEARCH, action_search),
            (&META_IMPACT, action_impact),
            (&META_HISTORY, action_history),
            (&META_REMEMBER, action_remember),
            (&META_EXPORT, action_export),
            (&META_STATS, action_stats),
            (&META_DOCTOR, action_doctor),
            (&META_READ, action_read),
            (&META_GRAPH, action_graph),
        ];
        Self { entries }
    }

    pub fn list(&self) -> Vec<&'static ActionMeta> {
        self.entries.iter().map(|(meta, _)| *meta).collect()
    }

    pub fn find(&self, name: &str) -> Option<(&'static ActionMeta, ActionHandler)> {
        for (meta, handler) in &self.entries {
            if meta.name == name || meta.aliases.contains(&name) {
                return Some((*meta, *handler));
            }
        }
        None
    }

    pub fn execute(&self, name: &str, ctx: &ActionContext, params: &ActionParams) -> Result<ActionResult> {
        match self.find(name) {
            Some((meta, handler)) => {
                if meta.read_only && params.get_bool("write") {
                    return Err(Error::InvalidArgument(format!(
                        "action `{}` is read-only", meta.name
                    )));
                }
                handler(ctx, params)
            }
            None => Err(Error::InvalidArgument(format!(
                "unknown action `{name}`; run `cse help` for available actions"
            ))),
        }
    }
}

impl Default for ActionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub static META_INIT: ActionMeta = ActionMeta {
    name: "init",
    description: "Initialize CodeSpace index in the current or specified directory",
    category: ActionCategory::Index,
    read_only: false,
    aliases: &[],
};

pub static META_UPDATE: ActionMeta = ActionMeta {
    name: "update",
    description: "Incrementally update the semantic graph index",
    category: ActionCategory::Index,
    read_only: false,
    aliases: &["sync"],
};

pub static META_CONTEXT: ActionMeta = ActionMeta {
    name: "context",
    description: "Build a ranked, token-budgeted context bundle for a query",
    category: ActionCategory::Context,
    read_only: true,
    aliases: &[],
};

pub static META_SEARCH: ActionMeta = ActionMeta {
    name: "search",
    description: "Search indexed symbols by name, path, or token",
    category: ActionCategory::Search,
    read_only: true,
    aliases: &["find"],
};

pub static META_IMPACT: ActionMeta = ActionMeta {
    name: "impact",
    description: "Analyze the blast radius of changes between two Git refs",
    category: ActionCategory::Impact,
    read_only: true,
    aliases: &[],
};

pub static META_HISTORY: ActionMeta = ActionMeta {
    name: "history",
    description: "Read prior engineering decisions",
    category: ActionCategory::Memory,
    read_only: true,
    aliases: &[],
};

pub static META_REMEMBER: ActionMeta = ActionMeta {
    name: "remember",
    description: "Store an engineering decision in the graph",
    category: ActionCategory::Memory,
    read_only: false,
    aliases: &[],
};

pub static META_EXPORT: ActionMeta = ActionMeta {
    name: "export",
    description: "Export the graph as JSON, DOT, or standalone HTML",
    category: ActionCategory::Export,
    read_only: true,
    aliases: &[],
};

pub static META_STATS: ActionMeta = ActionMeta {
    name: "stats",
    description: "Show index statistics and health summary",
    category: ActionCategory::System,
    read_only: true,
    aliases: &["status"],
};

pub static META_DOCTOR: ActionMeta = ActionMeta {
    name: "doctor",
    description: "Diagnose and optionally repair index issues",
    category: ActionCategory::System,
    read_only: false,
    aliases: &[],
};

pub static META_READ: ActionMeta = ActionMeta {
    name: "read",
    description: "Read a project file with path confinement and secret redaction",
    category: ActionCategory::System,
    read_only: true,
    aliases: &[],
};

pub static META_GRAPH: ActionMeta = ActionMeta {
    name: "graph",
    description: "Return the full graph snapshot (files, symbols, edges, decisions)",
    category: ActionCategory::Export,
    read_only: true,
    aliases: &[],
};

fn action_init(ctx: &ActionContext, params: &ActionParams) -> Result<ActionResult> {
    let force = params.get_bool("force");
    let graph = storage::initialize(&ctx.root, force)?;
    let sv = graph.index_revision;
    let msg = format!(
        "initialized CodeSpace index at {}",
        ctx.root.display()
    );
    Ok(ActionResult::ok(msg, sv))
}

fn action_update(ctx: &ActionContext, params: &ActionParams) -> Result<ActionResult> {
    let force = params.get_bool("force");
    let options = crate::indexer::IndexOptions {
        force,
        ..Default::default()
    };
    let stats = crate::indexer::build(&ctx.root, &options)?;
    let sv = stats.symbols as u64;
    let msg = if ctx.format == OutputFormat::Json {
        format!(
            "{{\"files_scanned\":{},\"files_indexed\":{},\"files_skipped\":{},\"files_removed\":{},\"symbols\":{},\"edges\":{},\"bytes_scanned\":{},\"elapsed_ms\":{}}}",
            stats.files_scanned, stats.files_indexed, stats.files_skipped_unchanged,
            stats.files_removed, stats.symbols, stats.edges, stats.bytes_scanned, stats.elapsed_ms
        )
    } else {
        format!(
            "scanned {} file(s), indexed {}, skipped {} unchanged, removed {}; {} symbol(s), {} edge(s), {} ms",
            stats.files_scanned, stats.files_indexed, stats.files_skipped_unchanged,
            stats.files_removed, stats.symbols, stats.edges, stats.elapsed_ms
        )
    };
    Ok(ActionResult::ok(msg, sv))
}

fn action_context(ctx: &ActionContext, params: &ActionParams) -> Result<ActionResult> {
    let query = params.first_or("").trim().to_string();
    if query.is_empty() {
        return Err(Error::InvalidArgument("query is required for context".to_string()));
    }
    let mut options = ContextOptions::default();
    if let Some(max_tokens) = params.get_usize("max-tokens") {
        options.max_tokens = max_tokens.clamp(128, 32_000);
    }
    if let Some(max_items) = params.get_usize("max-items") {
        options.max_items = max_items.clamp(1, 50);
    }
    let bundle = build_context(&ctx.root, &ctx.graph, &query, &options)?;
    let output = match ctx.format {
        OutputFormat::Json => render_context_json(&bundle),
        OutputFormat::Markdown => render_markdown(&bundle),
        OutputFormat::Plain => render_plain(&bundle),
    };
    Ok(ActionResult::ok(output, ctx.graph.index_revision))
}

fn action_search(ctx: &ActionContext, params: &ActionParams) -> Result<ActionResult> {
    let query = params.first_or("").trim().to_string();
    if query.is_empty() {
        return Err(Error::InvalidArgument("query is required for search".to_string()));
    }
    let kind = params.get("kind").and_then(SymbolKind::parse);
    let limit = params.get_usize("limit").unwrap_or(20).clamp(1, 200);
    let hits = find_symbols(&ctx.graph, &query, kind, limit);
    let output = if ctx.format == OutputFormat::Json {
        render_search_json(&ctx.graph, &hits)
    } else {
        render_search_plain(&ctx.graph, &hits)
    };
    Ok(ActionResult::ok(output, ctx.graph.index_revision))
}

fn action_impact(ctx: &ActionContext, params: &ActionParams) -> Result<ActionResult> {
    let from = params.get_or("from", "HEAD~1");
    let to = params.get_or("to", "HEAD");
    let depth = params.get_usize("depth").unwrap_or(3).clamp(1, 10);
    let report = impact::analyze(&ctx.root, &ctx.graph, &from, &to, depth)?;
    let output = if ctx.format == OutputFormat::Json {
        impact::render_json(&report)
    } else {
        impact::render_plain(&report)
    };
    Ok(ActionResult::ok(output, ctx.graph.index_revision))
}

fn action_history(ctx: &ActionContext, params: &ActionParams) -> Result<ActionResult> {
    let target = params.first_or("").trim().to_string();
    let limit = params.get_usize("limit").unwrap_or(10).clamp(1, 100);
    let decisions = memory::history(&ctx.graph, &target, limit);
    let output = if ctx.format == OutputFormat::Json {
        memory::render_history_json(&decisions)
    } else {
        memory::render_history_plain(&decisions)
    };
    Ok(ActionResult::ok(output, ctx.graph.index_revision))
}

fn action_remember(ctx: &ActionContext, params: &ActionParams) -> Result<ActionResult> {
    let summary = params.get_or("summary", "").trim().to_string();
    if summary.is_empty() {
        return Err(Error::InvalidArgument("summary is required for remember".to_string()));
    }
    let mut graph = ctx.graph.clone();
    let id = memory::remember(&mut graph, RememberInput {
        file: params.get_or("file", "").trim().to_string(),
        symbol: params.get_or("symbol", "").trim().to_string(),
        session: params.get_or("session", "").trim().to_string(),
        agent: params.get_or("agent", "").trim().to_string(),
        summary,
        rationale: params.get_or("rationale", "").trim().to_string(),
        tags: params.get("tags").map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default(),
    });
    storage::save(&ctx.root, &graph)?;
    let msg = format!("remembered decision {id}");
    Ok(ActionResult::ok(msg, graph.index_revision))
}

fn action_export(ctx: &ActionContext, params: &ActionParams) -> Result<ActionResult> {
    let format = params.get_or("format", "json").to_ascii_lowercase();
    let output = match format.as_str() {
        "json" => export::to_json(&ctx.graph),
        "dot" | "graphviz" => export::to_graphviz(&ctx.graph),
        "html" => export::to_html(&ctx.graph),
        other => return Err(Error::InvalidArgument(format!("unknown export format `{other}`"))),
    };
    Ok(ActionResult::ok(output, ctx.graph.index_revision))
}

fn action_stats(ctx: &ActionContext, _params: &ActionParams) -> Result<ActionResult> {
    let g = &ctx.graph;
    let output = if ctx.format == OutputFormat::Json {
        format!(
            "{{\"files\":{},\"symbols\":{},\"edges\":{},\"decisions\":{},\"index_revision\":{},\"schema_version\":{},\"updated_unix_ms\":{}}}",
            g.files.len(), g.symbols.len(), g.edges.len(), g.decisions.len(),
            g.index_revision, g.schema_version, g.updated_unix_ms
        )
    } else {
        format!(
            "files: {}\nsymbols: {}\nedges: {}\ndecisions: {}\nindex revision: {}\nschema: {}\nupdated: {}",
            g.files.len(), g.symbols.len(), g.edges.len(), g.decisions.len(),
            g.index_revision, g.schema_version, g.updated_unix_ms
        )
    };
    Ok(ActionResult::ok(output, g.index_revision))
}

fn action_doctor(ctx: &ActionContext, params: &ActionParams) -> Result<ActionResult> {
    let repair = params.get_bool("repair");
    let mut messages = Vec::new();
    let index_exists = crate::util::index_path(&ctx.root).exists();
    if !index_exists {
        messages.push("index file not found; run `cse init`".to_string());
    } else {
        match storage::load(&ctx.root) {
            Ok(g) => {
                messages.push(format!("index schema_version: {}", g.schema_version));
                messages.push(format!("index revision: {}", g.index_revision));
                messages.push(format!("files: {}, symbols: {}, edges: {}", g.files.len(), g.symbols.len(), g.edges.len()));
            }
            Err(error) => {
                messages.push(format!("index load error: {error}"));
                if repair {
                    match storage::repair(&ctx.root) {
                        Ok(actions) => {
                            for action in actions {
                                messages.push(format!("repair: {action}"));
                            }
                        }
                        Err(e) => messages.push(format!("repair failed: {e}")),
                    }
                }
            }
        }
    }
    let lock = crate::util::lock_path(&ctx.root);
    if lock.exists() {
        messages.push(format!("lock file present: {}", lock.display()));
        if repair {
            let _ = fs::remove_file(&lock);
            messages.push("repair: removed lock file".to_string());
        }
    }
    if repair && index_exists {
        match storage::repair(&ctx.root) {
            Ok(actions) => {
                for action in actions {
                    if !messages.iter().any(|m| m.contains(&action)) {
                        messages.push(format!("repair: {action}"));
                    }
                }
            }
            Err(e) => messages.push(format!("repair failed: {e}")),
        }
    }
    let output = messages.join("\n");
    Ok(ActionResult::ok(output, ctx.graph.index_revision))
}

fn action_read(ctx: &ActionContext, params: &ActionParams) -> Result<ActionResult> {
    let file = params.first_or("").trim().to_string();
    if file.is_empty() {
        return Err(Error::InvalidArgument("file path is required for read".to_string()));
    }
    if file.contains("..") {
        return Err(Error::InvalidArgument("path traversal is not allowed".to_string()));
    }
    let max_lines = params.get_usize("max-lines").unwrap_or(400).clamp(1, 5_000);
    let path = ctx.root.join(&file);
    let canonical = fs::canonicalize(&path)?;
    let canonical_root = fs::canonicalize(&ctx.root).unwrap_or_else(|_| ctx.root.to_path_buf());
    if !canonical.starts_with(&canonical_root) {
        return Err(Error::InvalidArgument("path escapes project root".to_string()));
    }
    let relative = normalized_relative(&ctx.root, &canonical)?;
    if relative == ".git" || relative == ".codespace"
        || relative.starts_with(".git/") || relative.starts_with(".codespace/")
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
    let result = format!(
        "file: {relative}\nredactions: {}\n{}",
        redacted.redactions, redacted.content
    );
    Ok(ActionResult::ok(result, ctx.graph.index_revision))
}

fn action_graph(ctx: &ActionContext, _params: &ActionParams) -> Result<ActionResult> {
    let output = if ctx.format == OutputFormat::Json {
        export::to_json(&ctx.graph)
    } else {
        format!(
            "files: {}, symbols: {}, edges: {}, decisions: {}",
            ctx.graph.files.len(), ctx.graph.symbols.len(), ctx.graph.edges.len(), ctx.graph.decisions.len()
        )
    };
    Ok(ActionResult::ok(output, ctx.graph.index_revision))
}

fn render_search_json(graph: &GraphIndex, hits: &[model::SearchHit]) -> String {
    let rows: Vec<String> = hits
        .iter()
        .filter_map(|hit| {
            let symbol = graph.symbols.get(&hit.symbol_id)?;
            let path = graph.file_for_symbol(symbol).map_or("", |f| f.path.as_str());
            Some(format!(
                "{{\"id\":{},\"name\":\"{}\",\"qualified_name\":\"{}\",\"kind\":\"{}\",\"path\":\"{}\",\"line_start\":{},\"line_end\":{},\"score_milli\":{},\"reasons\":[{}]}}",
                symbol.id,
                json_escape(&symbol.name),
                json_escape(&symbol.qualified_name),
                symbol.kind.as_str(),
                json_escape(path),
                symbol.line_start,
                symbol.line_end,
                hit.score_milli,
                hit.reasons.iter().map(|r| format!("\"{}\"", json_escape(r))).collect::<Vec<_>>().join(",")
            ))
        })
        .collect();
    format!("[{}]", rows.join(","))
}

fn render_search_plain(graph: &GraphIndex, hits: &[model::SearchHit]) -> String {
    if hits.is_empty() {
        return "No symbols found.\n".to_string();
    }
    let mut output = String::new();
    for hit in hits {
        if let Some(symbol) = graph.symbols.get(&hit.symbol_id) {
            let path = graph.file_for_symbol(symbol).map_or("", |f| f.path.as_str());
            output.push_str(&format!(
                "{} [{}] {}:{}-{} score={} reasons={}\n",
                symbol.qualified_name,
                symbol.kind.as_str(),
                path,
                symbol.line_start,
                symbol.line_end,
                hit.score_milli,
                hit.reasons.join(",")
            ));
        }
    }
    output
}
