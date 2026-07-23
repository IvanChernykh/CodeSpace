use crate::model::{Decision, Edge, EdgeKind, Error, FileRecord, GraphIndex, Result, Symbol, SymbolKind};
use crate::util::{escape_field, index_dir, index_path, lock_path, now_unix_ms, split_escaped_tsv};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

const MAGIC: &str = "CODESPACE\t2";
const MAGIC_V1: &str = "CODESPACE\t1";

pub struct WriteLock {
    path: PathBuf,
}

impl WriteLock {
    pub fn acquire(root: &Path, timeout: Duration) -> Result<Self> {
        fs::create_dir_all(index_dir(root))?;
        let path = lock_path(root);
        let deadline = Instant::now() + timeout;
        loop {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    writeln!(file, "{}\t{}", std::process::id(), now_unix_ms())?;
                    file.sync_all()?;
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if is_stale_lock(&path, Duration::from_secs(300)) {
                        let _ = fs::remove_file(&path);
                        continue;
                    }
                    if Instant::now() >= deadline {
                        return Err(Error::InvalidArgument(format!(
                            "index is locked: {}. Run `cse doctor --repair` if no writer is active",
                            path.display()
                        )));
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}

impl Drop for WriteLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn is_stale_lock(path: &Path, max_age: Duration) -> bool {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age > max_age)
}

pub fn initialize(root: &Path, force: bool) -> Result<GraphIndex> {
    let directory = index_dir(root);
    if directory.exists() && !force && index_path(root).exists() {
        return Err(Error::InvalidArgument(format!(
            "index already exists at {}; pass --force to rebuild",
            directory.display()
        )));
    }
    fs::create_dir_all(&directory)?;
    let now = now_unix_ms();
    let graph = GraphIndex::empty(root.to_string_lossy().to_string(), now);
    save(root, &graph)?;
    Ok(graph)
}

pub fn load(root: &Path) -> Result<GraphIndex> {
    let path = index_path(root);
    if !path.exists() {
        return Err(Error::NotInitialized(root.to_path_buf()));
    }
    let file = File::open(&path)?;
    let mut reader = BufReader::new(file);
    let mut first = String::new();
    reader.read_line(&mut first)?;
    let header = first.trim_end();
    let is_v1 = header == MAGIC_V1;
    if header != MAGIC && !is_v1 {
        return Err(Error::CorruptIndex(format!(
            "unsupported header in {}",
            path.display()
        )));
    }

    let mut graph = GraphIndex::empty(String::new(), 0);
    for (line_number, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        if line.trim().is_empty() {
            continue;
        }
        let fields = split_escaped_tsv(&line)?;
        let record = fields.first().map_or("", String::as_str);
        match record {
            "META" => parse_meta(&mut graph, &fields, line_number + 2)?,
            "FILE" => parse_file(&mut graph, &fields, line_number + 2)?,
            "SYMBOL" => parse_symbol(&mut graph, &fields, line_number + 2)?,
            "EDGE" => parse_edge(&mut graph, &fields, line_number + 2)?,
            "DECISION" => parse_decision(&mut graph, &fields, line_number + 2)?,
            other => {
                return Err(Error::CorruptIndex(format!(
                    "unknown record `{other}` at line {}",
                    line_number + 2
                )));
            }
        }
    }
    if graph.project_root.is_empty() {
        graph.project_root = root.to_string_lossy().to_string();
    }
    if is_v1 {
        graph.schema_version = 2;
    }
    graph.rebuild_indexes();
    Ok(graph)
}

pub fn save(root: &Path, graph: &GraphIndex) -> Result<()> {
    fs::create_dir_all(index_dir(root))?;
    let _lock = WriteLock::acquire(root, Duration::from_secs(3))?;
    let path = index_path(root);
    let temporary = path.with_extension(format!("tmp.{}", std::process::id()));
    {
        let file = File::create(&temporary)?;
        let mut writer = BufWriter::new(file);
        writeln!(writer, "{MAGIC}")?;
        writeln!(
            writer,
            "META\t{}\t{}\t{}\t{}\t{}",
            graph.schema_version,
            escape_field(&graph.project_root),
            graph.created_unix_ms,
            graph.updated_unix_ms,
            graph.index_revision
        )?;
        for file in graph.files.values() {
            writeln!(
                writer,
                "FILE\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                file.id,
                escape_field(&file.path),
                escape_field(&file.language),
                file.hash,
                file.bytes,
                file.modified_unix_ms,
                file.line_count
            )?;
        }
        for symbol in graph.symbols.values() {
            writeln!(
                writer,
                "SYMBOL\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                symbol.id,
                symbol.file_id,
                escape_field(&symbol.name),
                escape_field(&symbol.qualified_name),
                symbol.kind.as_str(),
                symbol.line_start,
                symbol.line_end,
                escape_field(&symbol.signature),
                escape_field(&symbol.doc),
                symbol.complexity
            )?;
        }
        for edge in &graph.edges {
            writeln!(
                writer,
                "EDGE\t{}\t{}\t{}\t{}\t{}\t{}",
                edge.from,
                edge.to,
                edge.kind.as_str(),
                edge.confidence_milli,
                edge.precision.as_str(),
                escape_field(&edge.evidence)
            )?;
        }
        for decision in graph.decisions.values() {
            writeln!(
                writer,
                "DECISION\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                decision.id,
                decision.timestamp_unix_ms,
                escape_field(&decision.file),
                escape_field(&decision.symbol),
                escape_field(&decision.session),
                escape_field(&decision.agent),
                escape_field(&decision.summary),
                escape_field(&decision.rationale),
                escape_field(&decision.tags.join(","))
            )?;
        }
        writer.flush()?;
        writer.get_ref().sync_all()?;
    }
    replace_index_file(&temporary, &path)?;
    if let Some(parent) = path.parent() {
        if let Ok(directory) = File::open(parent) {
            let _ = directory.sync_all();
        }
    }
    Ok(())
}


fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("csf.bak")
}

fn replace_index_file(temporary: &Path, destination: &Path) -> Result<()> {
    #[cfg(not(windows))]
    {
        fs::rename(temporary, destination)?;
        Ok(())
    }
    #[cfg(windows)]
    {
        let backup = backup_path(destination);
        if backup.exists() {
            fs::remove_file(&backup)?;
        }
        if destination.exists() {
            fs::rename(destination, &backup)?;
        }
        match fs::rename(temporary, destination) {
            Ok(()) => {
                if backup.exists() {
                    fs::remove_file(backup)?;
                }
                Ok(())
            }
            Err(error) => {
                if backup.exists() && !destination.exists() {
                    let _ = fs::rename(&backup, destination);
                }
                Err(error.into())
            }
        }
    }
}

pub fn repair(root: &Path) -> Result<Vec<String>> {
    let mut actions = Vec::new();
    let lock = lock_path(root);
    if lock.exists() && is_stale_lock(&lock, Duration::from_secs(5)) {
        fs::remove_file(&lock)?;
        actions.push(format!("removed stale lock {}", lock.display()));
    }
    let path = index_path(root);
    let backup = backup_path(&path);
    if backup.exists() && !path.exists() {
        fs::rename(&backup, &path)?;
        actions.push(format!("restored backup index {}", path.display()));
    } else if backup.exists() && path.exists() {
        fs::remove_file(&backup)?;
        actions.push(format!("removed obsolete backup {}", backup.display()));
    }
    let directory = index_dir(root);
    if directory.exists() {
        for entry_result in fs::read_dir(&directory)? {
            let entry = entry_result?;
            let path = entry.path();
            if path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().contains(".tmp."))
            {
                fs::remove_file(&path)?;
                actions.push(format!("removed temporary file {}", path.display()));
            }
        }
    }
    if actions.is_empty() {
        actions.push("no repair action required".to_string());
    }
    Ok(actions)
}

fn parse_meta(graph: &mut GraphIndex, fields: &[String], line: usize) -> Result<()> {
    if fields.len() == 5 {
        graph.schema_version = parse_number(&fields[1], line, "schema_version")?;
        graph.project_root = fields[2].clone();
        graph.created_unix_ms = parse_number(&fields[3], line, "created_unix_ms")?;
        graph.updated_unix_ms = parse_number(&fields[4], line, "updated_unix_ms")?;
        graph.index_revision = 0;
    } else if fields.len() == 6 {
        graph.schema_version = parse_number(&fields[1], line, "schema_version")?;
        graph.project_root = fields[2].clone();
        graph.created_unix_ms = parse_number(&fields[3], line, "created_unix_ms")?;
        graph.updated_unix_ms = parse_number(&fields[4], line, "updated_unix_ms")?;
        graph.index_revision = parse_number(&fields[5], line, "index_revision")?;
    } else {
        return Err(Error::CorruptIndex(format!(
            "expected 5 or 6 META fields, got {} at line {line}",
            fields.len()
        )));
    }
    Ok(())
}

fn parse_file(graph: &mut GraphIndex, fields: &[String], line: usize) -> Result<()> {
    require_fields(fields, 8, line)?;
    let file = FileRecord {
        id: parse_number(&fields[1], line, "file.id")?,
        path: fields[2].clone(),
        language: fields[3].clone(),
        hash: parse_number(&fields[4], line, "file.hash")?,
        bytes: parse_number(&fields[5], line, "file.bytes")?,
        modified_unix_ms: parse_number(&fields[6], line, "file.modified")?,
        line_count: parse_number(&fields[7], line, "file.lines")?,
    };
    graph.files.insert(file.id, file);
    Ok(())
}

fn parse_symbol(graph: &mut GraphIndex, fields: &[String], line: usize) -> Result<()> {
    require_fields(fields, 11, line)?;
    let kind = SymbolKind::parse(&fields[5]).ok_or_else(|| {
        Error::CorruptIndex(format!("invalid symbol kind `{}` at line {line}", fields[5]))
    })?;
    let symbol = Symbol {
        id: parse_number(&fields[1], line, "symbol.id")?,
        file_id: parse_number(&fields[2], line, "symbol.file_id")?,
        name: fields[3].clone(),
        qualified_name: fields[4].clone(),
        kind,
        line_start: parse_number(&fields[6], line, "symbol.line_start")?,
        line_end: parse_number(&fields[7], line, "symbol.line_end")?,
        signature: fields[8].clone(),
        doc: fields[9].clone(),
        complexity: parse_number(&fields[10], line, "symbol.complexity")?,
    };
    graph.symbols.insert(symbol.id, symbol);
    Ok(())
}

fn parse_edge(graph: &mut GraphIndex, fields: &[String], line: usize) -> Result<()> {
    let kind = EdgeKind::parse(&fields[3]).ok_or_else(|| {
        Error::CorruptIndex(format!("invalid edge kind `{}` at line {line}", fields[3]))
    })?;
    let (precision, evidence) = if fields.len() >= 7 {
        let p = crate::model::PrecisionTier::parse(&fields[5]).unwrap_or_default();
        (p, fields[6].clone())
    } else {
        (crate::model::PrecisionTier::Parser, String::new())
    };
    graph.edges.insert(Edge {
        from: parse_number(&fields[1], line, "edge.from")?,
        to: parse_number(&fields[2], line, "edge.to")?,
        kind,
        confidence_milli: parse_number(&fields[4], line, "edge.confidence")?,
        precision,
        evidence,
    });
    Ok(())
}

fn parse_decision(graph: &mut GraphIndex, fields: &[String], line: usize) -> Result<()> {
    require_fields(fields, 10, line)?;
    let decision = Decision {
        id: parse_number(&fields[1], line, "decision.id")?,
        timestamp_unix_ms: parse_number(&fields[2], line, "decision.timestamp")?,
        file: fields[3].clone(),
        symbol: fields[4].clone(),
        session: fields[5].clone(),
        agent: fields[6].clone(),
        summary: fields[7].clone(),
        rationale: fields[8].clone(),
        tags: fields[9]
            .split(',')
            .filter(|tag| !tag.trim().is_empty())
            .map(|tag| tag.trim().to_string())
            .collect(),
    };
    graph.decisions.insert(decision.id, decision);
    Ok(())
}

fn require_fields(fields: &[String], expected: usize, line: usize) -> Result<()> {
    if fields.len() != expected {
        return Err(Error::CorruptIndex(format!(
            "expected {expected} fields, got {} at line {line}",
            fields.len()
        )));
    }
    Ok(())
}

fn parse_number<T>(value: &str, line: usize, field: &str) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value.parse::<T>().map_err(|error| {
        Error::CorruptIndex(format!("invalid {field} at line {line}: {error}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GraphIndex;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn round_trip_index() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let root = std::env::temp_dir().join(format!("codespace-storage-{suffix}"));
        fs::create_dir_all(&root).unwrap_or_else(|error| panic!("create temp dir: {error}"));
        let graph = GraphIndex::empty(root.to_string_lossy().to_string(), 1);
        save(&root, &graph).unwrap_or_else(|error| panic!("save graph: {error}"));
        let loaded = load(&root).unwrap_or_else(|error| panic!("load graph: {error}"));
        assert_eq!(loaded.schema_version, 2);
        let path = index_path(&root);
        let backup = backup_path(&path);
        fs::rename(&path, &backup)
            .unwrap_or_else(|error| panic!("simulate interrupted replacement: {error}"));
        let actions = repair(&root).unwrap_or_else(|error| panic!("repair backup: {error}"));
        assert!(actions.iter().any(|action| action.contains("restored backup index")));
        load(&root).unwrap_or_else(|error| panic!("load repaired graph: {error}"));
        let _ = fs::remove_dir_all(root);
    }
}
