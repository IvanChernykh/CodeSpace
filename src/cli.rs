use crate::context::{self, ContextOptions};
use crate::export;
use crate::impact;
use crate::indexer::{self, IndexOptions};
use crate::memory::{self, RememberInput};
use crate::model::{Error, Result, SymbolKind};
use crate::search::find_symbols;
use crate::storage;
use crate::util::{canonical_root, index_path, json_escape, now_unix_ms};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run<I>(args: I) -> Result<i32>
where
    I: IntoIterator<Item = String>,
{
    let args: Vec<String> = args.into_iter().collect();
    if args.len() <= 1 {
        print_help();
        return Ok(0);
    }
    let command = args[1].as_str();
    let parsed = ParsedArgs::parse(&args[2..])?;
    match command {
        "init" => command_init(&parsed),
        "update" | "sync" => command_update(&parsed),
        "context" => command_context(&parsed),
        "find" | "search" => command_find(&parsed),
        "impact" => command_impact(&parsed),
        "history" => command_history(&parsed),
        "remember" => command_remember(&parsed),
        "serve" => command_serve(&parsed),
        "export" => command_export(&parsed),
        "stats" | "status" => command_stats(&parsed),
        "doctor" => command_doctor(&parsed),
        "benchmark" | "bench" => command_benchmark(&parsed),
        "shell" => command_shell(&parsed),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(0)
        }
        "version" | "--version" | "-V" => {
            println!("cse {VERSION}");
            Ok(0)
        }
        other => Err(Error::InvalidArgument(format!(
            "unknown command `{other}`; run `cse help`"
        ))),
    }
}

fn command_init(args: &ParsedArgs) -> Result<i32> {
    let root = root_from_args(args)?;
    let force = args.flag("force");
    if force && root.join(".codespace").exists() {
        fs::remove_dir_all(root.join(".codespace"))?;
    }
    storage::initialize(&root, false)?;
    let stats = indexer::build(
        &root,
        &IndexOptions {
            force: true,
            ..IndexOptions::default()
        },
    )?;
    println!(
        "Initialized {}\nIndexed {} files, {} symbols, {} edges in {} ms",
        root.display(),
        stats.files_indexed,
        stats.symbols,
        stats.edges,
        stats.elapsed_ms
    );
    Ok(0)
}

fn command_update(args: &ParsedArgs) -> Result<i32> {
    let root = root_from_args(args)?;
    ensure_initialized(&root)?;
    let options = IndexOptions {
        force: args.flag("force"),
        max_file_bytes: args
            .value("max-file-bytes")
            .and_then(|value| value.parse().ok())
            .unwrap_or(crate::util::DEFAULT_MAX_FILE_BYTES),
        follow_symlinks: args.flag("follow-symlinks"),
    };
    if args.flag("watch") {
        let interval_ms = args
            .value("interval-ms")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(750)
            .clamp(100, 60_000);
        indexer::watch(&root, &options, Duration::from_millis(interval_ms))?;
        return Ok(0);
    }
    let stats = indexer::build(&root, &options)?;
    println!(
        "Updated: scanned={}, indexed={}, unchanged={}, removed={}, symbols={}, edges={}, bytes={}, elapsed_ms={}",
        stats.files_scanned,
        stats.files_indexed,
        stats.files_skipped_unchanged,
        stats.files_removed,
        stats.symbols,
        stats.edges,
        stats.bytes_scanned,
        stats.elapsed_ms
    );
    Ok(0)
}

fn command_context(args: &ParsedArgs) -> Result<i32> {
    let root = root_from_args(args)?;
    let graph = storage::load(&root)?;
    let query = args
        .value("query")
        .cloned()
        .or_else(|| args.positionals.first().cloned())
        .ok_or_else(|| Error::InvalidArgument("context requires --query <text>".to_string()))?;
    let options = ContextOptions {
        max_tokens: args
            .value("max-tokens")
            .and_then(|value| value.parse().ok())
            .unwrap_or(1_200)
            .clamp(128, 32_000),
        max_items: args
            .value("max-items")
            .and_then(|value| value.parse().ok())
            .unwrap_or(8)
            .clamp(1, 50),
        include_docs: !args.flag("no-docs"),
        redact_secrets: !args.flag("no-redact"),
        neighbor_lines: args
            .value("neighbor-lines")
            .and_then(|value| value.parse().ok())
            .unwrap_or(2)
            .min(20),
    };
    let bundle = context::build_context(&root, &graph, &query, &options)?;
    let format = args.value("format").map_or("markdown", String::as_str);
    let rendered = match format {
        "json" => context::render_json(&bundle),
        "plain" => context::render_plain(&bundle),
        "markdown" | "md" => context::render_markdown(&bundle),
        other => {
            return Err(Error::InvalidArgument(format!(
                "unsupported context format `{other}`"
            )));
        }
    };
    write_output(args, &rendered)?;
    Ok((bundle.items.is_empty()) as i32)
}

fn command_find(args: &ParsedArgs) -> Result<i32> {
    let root = root_from_args(args)?;
    let graph = storage::load(&root)?;
    let query = args
        .positionals
        .first()
        .cloned()
        .or_else(|| args.value("query").cloned())
        .ok_or_else(|| Error::InvalidArgument("find requires <symbol>".to_string()))?;
    let kind = args
        .value("type")
        .or_else(|| args.value("kind"))
        .map(|value| {
            SymbolKind::parse(value).ok_or_else(|| {
                Error::InvalidArgument(format!("unknown symbol type `{value}`"))
            })
        })
        .transpose()?;
    let limit = args
        .value("limit")
        .and_then(|value| value.parse().ok())
        .unwrap_or(20)
        .clamp(1, 500);
    let hits = find_symbols(&graph, &query, kind, limit);
    let format = args.value("format").map_or("plain", String::as_str);
    let rendered = if format == "json" {
        let rows = hits
            .iter()
            .filter_map(|hit| graph.symbols.get(&hit.symbol_id).map(|symbol| (hit, symbol)))
            .map(|(hit, symbol)| {
                let path = graph.file_for_symbol(symbol).map_or("", |file| file.path.as_str());
                format!(
                    "{{\"id\":{},\"name\":\"{}\",\"qualified_name\":\"{}\",\"kind\":\"{}\",\"path\":\"{}\",\"line_start\":{},\"line_end\":{},\"score_milli\":{},\"reasons\":[{}]}}",
                    symbol.id,
                    json_escape(&symbol.name),
                    json_escape(&symbol.qualified_name),
                    symbol.kind.as_str(),
                    json_escape(path),
                    symbol.line_start,
                    symbol.line_end,
                    hit.score_milli,
                    hit.reasons.iter().map(|reason| format!("\"{}\"", json_escape(reason))).collect::<Vec<_>>().join(",")
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!("[{rows}]")
    } else if format == "plain" {
        let mut output = String::new();
        for hit in &hits {
            let Some(symbol) = graph.symbols.get(&hit.symbol_id) else {
                continue;
            };
            let path = graph.file_for_symbol(symbol).map_or("", |file| file.path.as_str());
            output.push_str(&format!(
                "{}\t{}\t{}:{}-{}\tscore={}\t{}\n",
                symbol.kind.as_str(),
                symbol.qualified_name,
                path,
                symbol.line_start,
                symbol.line_end,
                hit.score_milli,
                hit.reasons.join(",")
            ));
        }
        output
    } else {
        return Err(Error::InvalidArgument(format!("unsupported format `{format}`")));
    };
    write_output(args, &rendered)?;
    Ok((hits.is_empty()) as i32)
}

fn command_impact(args: &ParsedArgs) -> Result<i32> {
    let root = root_from_args(args)?;
    let graph = storage::load(&root)?;
    let from = args.value("from").map_or("HEAD~1", String::as_str);
    let to = args.value("to").map_or("HEAD", String::as_str);
    let depth = args
        .value("depth")
        .and_then(|value| value.parse().ok())
        .unwrap_or(3)
        .clamp(1, 10);
    let report = impact::analyze(&root, &graph, from, to, depth)?;
    let rendered = match args.value("format").map_or("plain", String::as_str) {
        "plain" => impact::render_plain(&report),
        "json" => impact::render_json(&report),
        other => return Err(Error::InvalidArgument(format!("unsupported format `{other}`"))),
    };
    write_output(args, &rendered)?;
    Ok((report.risk_score >= 80) as i32)
}

fn command_history(args: &ParsedArgs) -> Result<i32> {
    let root = root_from_args(args)?;
    let graph = storage::load(&root)?;
    let target = args.positionals.first().map_or("", String::as_str);
    let limit = args
        .value("limit")
        .and_then(|value| value.parse().ok())
        .unwrap_or(10)
        .clamp(1, 1_000);
    let decisions = memory::history(&graph, target, limit);
    let rendered = match args.value("format").map_or("plain", String::as_str) {
        "plain" => memory::render_history_plain(&decisions),
        "json" => memory::render_history_json(&decisions),
        other => return Err(Error::InvalidArgument(format!("unsupported format `{other}`"))),
    };
    write_output(args, &rendered)?;
    Ok((decisions.is_empty()) as i32)
}

fn command_remember(args: &ParsedArgs) -> Result<i32> {
    let root = root_from_args(args)?;
    let mut graph = storage::load(&root)?;
    let summary = args
        .value("summary")
        .cloned()
        .or_else(|| args.positionals.first().cloned())
        .ok_or_else(|| Error::InvalidArgument("remember requires --summary <text>".to_string()))?;
    let input = RememberInput {
        file: args.value("file").cloned().unwrap_or_default(),
        symbol: args.value("symbol").cloned().unwrap_or_default(),
        session: args
            .value("session")
            .cloned()
            .unwrap_or_else(|| format!("session-{}", now_unix_ms())),
        agent: args.value("agent").cloned().unwrap_or_else(|| "unknown".to_string()),
        summary,
        rationale: args.value("rationale").cloned().unwrap_or_default(),
        tags: args
            .value("tags")
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|tag| !tag.is_empty())
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
    };
    let id = memory::remember(&mut graph, input);
    storage::save(&root, &graph)?;
    println!("Recorded decision {id}");
    Ok(0)
}

fn command_serve(args: &ParsedArgs) -> Result<i32> {
    let root = root_from_args(args)?;
    let graph = storage::load(&root)?;
    if args.flag("mcp") || (!args.flag("rest") && args.value("port").is_none()) {
        crate::mcp::serve(&root, graph)?;
        return Ok(0);
    }
    let host = args.value("host").map_or("127.0.0.1", String::as_str);
    let port = args
        .value("port")
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8080);
    if host != "127.0.0.1" && host != "localhost" && !args.flag("allow-remote") {
        return Err(Error::InvalidArgument(
            "remote REST binding requires --allow-remote; no authentication is built in".to_string(),
        ));
    }
    crate::rest::serve(&root, graph, &format!("{host}:{port}"))?;
    Ok(0)
}

fn command_export(args: &ParsedArgs) -> Result<i32> {
    let root = root_from_args(args)?;
    let graph = storage::load(&root)?;
    let format = args.value("format").map_or("json", String::as_str);
    let rendered = match format {
        "json" => export::to_json(&graph),
        "graphviz" | "dot" => export::to_graphviz(&graph),
        "html" => export::to_html(&graph),
        other => return Err(Error::InvalidArgument(format!("unsupported export format `{other}`"))),
    };
    write_output(args, &rendered)?;
    Ok(0)
}

fn command_stats(args: &ParsedArgs) -> Result<i32> {
    let root = root_from_args(args)?;
    let graph = storage::load(&root)?;
    let index_bytes = fs::metadata(index_path(&root)).map_or(0, |metadata| metadata.len());
    let languages = graph.files.values().fold(BTreeMap::<String, usize>::new(), |mut map, file| {
        *map.entry(file.language.clone()).or_default() += 1;
        map
    });
    if args.value("format").is_some_and(|format| format == "json") || args.flag("json") {
        let language_json = languages
            .iter()
            .map(|(language, count)| format!("\"{}\":{}", json_escape(language), count))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "{{\"project_root\":\"{}\",\"files\":{},\"symbols\":{},\"edges\":{},\"decisions\":{},\"index_bytes\":{},\"updated_unix_ms\":{},\"languages\":{{{}}}}}",
            json_escape(&graph.project_root),
            graph.files.len(),
            graph.symbols.len(),
            graph.edges.len(),
            graph.decisions.len(),
            index_bytes,
            graph.updated_unix_ms,
            language_json
        );
    } else {
        println!("Project: {}", graph.project_root);
        println!("Files: {}", graph.files.len());
        println!("Symbols: {}", graph.symbols.len());
        println!("Edges: {}", graph.edges.len());
        println!("Decisions: {}", graph.decisions.len());
        println!("Index bytes: {index_bytes}");
        println!("Updated unix ms: {}", graph.updated_unix_ms);
        println!("Languages:");
        for (language, count) in languages {
            println!("  {language}: {count}");
        }
    }
    Ok(0)
}

fn command_doctor(args: &ParsedArgs) -> Result<i32> {
    let root = root_from_args(args)?;
    if args.flag("repair") {
        for action in storage::repair(&root)? {
            println!("REPAIR: {action}");
        }
    }
    let mut failures = Vec::new();
    if !root.join(".git").exists() {
        failures.push("not a Git repository; impact analysis will be unavailable".to_string());
    }
    if !index_path(&root).exists() {
        failures.push("index missing; run `cse init`".to_string());
    } else if let Err(error) = storage::load(&root) {
        failures.push(format!("index cannot be loaded: {error}"));
    }
    let git_available = std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    if !git_available {
        failures.push("git executable not found".to_string());
    }
    if failures.is_empty() {
        println!("OK: project, index, and Git integration are operational");
        Ok(0)
    } else {
        for failure in &failures {
            println!("WARN: {failure}");
        }
        Ok(1)
    }
}

fn command_benchmark(args: &ParsedArgs) -> Result<i32> {
    let root = root_from_args(args)?;
    let graph = storage::load(&root)?;
    let query = args.value("query").map_or("architecture context impact", String::as_str);
    let iterations = args
        .value("iterations")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(20)
        .clamp(1, 10_000);
    let options = ContextOptions::default();
    let started = Instant::now();
    let mut token_total = 0_usize;
    let mut bytes_total = 0_usize;
    for _ in 0..iterations {
        let bundle = context::build_context(&root, &graph, query, &options)?;
        token_total = token_total.saturating_add(bundle.estimated_tokens);
        bytes_total = bytes_total.saturating_add(bundle.returned_bytes);
    }
    let elapsed = started.elapsed();
    let elapsed_ms = elapsed.as_millis();
    let average_micros = elapsed.as_micros() / iterations as u128;
    println!(
        "iterations={iterations}\nelapsed_ms={elapsed_ms}\navg_query_us={average_micros}\navg_estimated_tokens={}\navg_returned_bytes={}\nfiles={}\nsymbols={}\nedges={}",
        token_total / iterations,
        bytes_total / iterations,
        graph.files.len(),
        graph.symbols.len(),
        graph.edges.len()
    );
    Ok(0)
}

fn command_shell(args: &ParsedArgs) -> Result<i32> {
    let root = root_from_args(args)?;
    ensure_initialized(&root)?;
    eprintln!("CodeSpace shell. Commands: find <q>, context <q>, stats, update, quit");
    let mut line = String::new();
    loop {
        print!("cse> ");
        io::stdout().flush()?;
        line.clear();
        if io::stdin().read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = line.trim();
        if matches!(trimmed, "quit" | "exit") {
            break;
        }
        if trimmed.is_empty() {
            continue;
        }
        let mut words = split_shell_words(trimmed)?;
        if words.is_empty() {
            continue;
        }
        let command = words.remove(0);
        let mut nested = vec!["cse".to_string(), command];
        nested.extend(words);
        nested.push("--path".to_string());
        nested.push(root.to_string_lossy().to_string());
        if let Err(error) = run(nested) {
            eprintln!("error: {error}");
        }
    }
    Ok(0)
}

fn root_from_args(args: &ParsedArgs) -> Result<PathBuf> {
    let path = args.value("path").map_or_else(|| PathBuf::from("."), |value| PathBuf::from(value));
    canonical_root(&path)
}

fn ensure_initialized(root: &Path) -> Result<()> {
    if !index_path(root).exists() {
        return Err(Error::NotInitialized(root.to_path_buf()));
    }
    Ok(())
}

fn write_output(args: &ParsedArgs, content: &str) -> Result<()> {
    if let Some(path) = args.value("output") {
        fs::write(path, content)?;
        println!("Wrote {path}");
    } else {
        println!("{content}");
    }
    Ok(())
}

#[derive(Debug, Default)]
struct ParsedArgs {
    values: BTreeMap<String, String>,
    flags: BTreeSet<String>,
    positionals: Vec<String>,
}

impl ParsedArgs {
    fn parse(args: &[String]) -> Result<Self> {
        let mut parsed = Self::default();
        let mut index = 0;
        while index < args.len() {
            let argument = &args[index];
            if argument == "--" {
                parsed.positionals.extend_from_slice(&args[index + 1..]);
                break;
            }
            if let Some(long) = argument.strip_prefix("--") {
                if let Some((key, value)) = long.split_once('=') {
                    parsed.values.insert(key.to_string(), value.to_string());
                } else if args.get(index + 1).is_some_and(|next| !next.starts_with('-'))
                    && option_expects_value(long)
                {
                    index += 1;
                    parsed.values.insert(long.to_string(), args[index].clone());
                } else {
                    parsed.flags.insert(long.to_string());
                }
            } else if argument.starts_with('-') && argument.len() == 2 {
                let key = match argument.as_str() {
                    "-p" => "path",
                    "-q" => "query",
                    "-f" => "format",
                    "-o" => "output",
                    "-h" => "help",
                    "-V" => "version",
                    other => {
                        return Err(Error::InvalidArgument(format!("unknown short option `{other}`")));
                    }
                };
                if matches!(key, "help" | "version") {
                    parsed.flags.insert(key.to_string());
                } else {
                    index += 1;
                    let value = args.get(index).ok_or_else(|| {
                        Error::InvalidArgument(format!("option `{argument}` requires a value"))
                    })?;
                    parsed.values.insert(key.to_string(), value.clone());
                }
            } else {
                parsed.positionals.push(argument.clone());
            }
            index += 1;
        }
        Ok(parsed)
    }

    fn flag(&self, key: &str) -> bool {
        self.flags.contains(key)
    }

    fn value(&self, key: &str) -> Option<&String> {
        self.values.get(key)
    }
}

fn option_expects_value(key: &str) -> bool {
    matches!(
        key,
        "path"
            | "query"
            | "format"
            | "output"
            | "type"
            | "kind"
            | "limit"
            | "from"
            | "to"
            | "depth"
            | "port"
            | "host"
            | "max-tokens"
            | "max-items"
            | "neighbor-lines"
            | "max-file-bytes"
            | "interval-ms"
            | "file"
            | "symbol"
            | "summary"
            | "rationale"
            | "session"
            | "agent"
            | "tags"
            | "iterations"
    )
}

fn split_shell_words(input: &str) -> Result<Vec<String>> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in input.chars() {
        if escaped {
            current.push(character);
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
            } else {
                current.push(character);
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
        } else if character.is_whitespace() {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
        } else {
            current.push(character);
        }
    }
    if quote.is_some() {
        return Err(Error::InvalidArgument("unterminated quote".to_string()));
    }
    if escaped {
        current.push('\\');
    }
    if !current.is_empty() {
        words.push(current);
    }
    Ok(words)
}

fn print_help() {
    println!(
        r#"CodeSpace {VERSION}
Local-first semantic code graph and compact AI context engine.

USAGE
  cse <command> [options]

CORE COMMANDS
  cse init [--path .] [--force]
  cse update [--path .] [--watch] [--force]
  cse context --query <text> [--format markdown|json|plain] [--max-tokens 1200]
  cse find <symbol> [--type function|class|...] [--format plain|json]
  cse impact [--from HEAD~1] [--to HEAD] [--depth 3] [--format plain|json]
  cse history [target] [--limit 10] [--format plain|json]
  cse remember --summary <text> [--file path] [--symbol name] [--rationale text]
  cse serve --mcp
  cse serve --rest [--host 127.0.0.1] [--port 8080]
  cse export --format json|html|graphviz [--output file]
  cse stats [--json]
  cse doctor [--repair]
  cse benchmark [--query text] [--iterations 20]
  cse shell

SECURITY DEFAULTS
  Source remains local. Secret-like values are redacted from context and MCP reads.
  Symlinks are ignored. Files over 1 MiB and common generated/vendor directories are skipped.
  REST binds to loopback; remote binding requires --allow-remote and has no authentication.
"#
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mixed_options() {
        let args = vec![
            "query".to_string(),
            "--format=json".to_string(),
            "--max-tokens".to_string(),
            "900".to_string(),
            "--watch".to_string(),
        ];
        let parsed = ParsedArgs::parse(&args).unwrap_or_else(|error| panic!("parse args: {error}"));
        assert_eq!(parsed.positionals, vec!["query"]);
        assert_eq!(parsed.value("format").map(String::as_str), Some("json"));
        assert!(parsed.flag("watch"));
    }
}
