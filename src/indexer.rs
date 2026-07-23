use crate::model::{Error, GraphIndex, IndexStats, Result};
use crate::parser::{parse_source, resolve_cross_file_edges, ParsedFile};
use crate::storage;
use crate::util::{
    is_probably_binary, normalized_relative, now_unix_ms, path_matches_pattern, read_ignore_patterns,
    DEFAULT_MAX_FILE_BYTES,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct IndexOptions {
    pub force: bool,
    pub max_file_bytes: u64,
    pub follow_symlinks: bool,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            force: false,
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            follow_symlinks: false,
        }
    }
}

pub fn build(root: &Path, options: &IndexOptions) -> Result<IndexStats> {
    let started = Instant::now();
    let mut graph = match storage::load(root) {
        Ok(graph) => graph,
        Err(Error::NotInitialized(_)) => storage::initialize(root, false)?,
        Err(error) => return Err(error),
    };
    let patterns = read_ignore_patterns(root);
    let files = collect_source_files(root, &patterns, options)?;
    let current_paths: BTreeSet<String> = files
        .iter()
        .filter_map(|path| normalized_relative(root, path).ok())
        .collect();

    let existing_paths: Vec<String> = graph.files_by_path.keys().cloned().collect();
    let mut removed = 0;
    for path in existing_paths {
        if !current_paths.contains(&path) {
            graph.remove_file(&path);
            removed += 1;
        }
    }

    let mut parsed_files = Vec::new();
    let mut files_scanned = 0;
    let mut files_indexed = 0;
    let mut files_skipped_unchanged = 0;
    let mut bytes_scanned = 0_u64;

    for path in files {
        files_scanned += 1;
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if metadata.len() > options.max_file_bytes {
            continue;
        }
        let relative = normalized_relative(root, &path)?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        bytes_scanned = bytes_scanned.saturating_add(bytes.len() as u64);
        if is_probably_binary(&bytes) {
            continue;
        }
        let source = String::from_utf8_lossy(&bytes);
        let hash = crate::util::stable_hash(source.as_bytes());
        if !options.force
            && graph
                .files_by_path
                .get(&relative)
                .and_then(|id| graph.files.get(id))
                .is_some_and(|file| file.hash == hash)
        {
            files_skipped_unchanged += 1;
            continue;
        }
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_millis());
        if let Some(parsed) = parse_source(&relative, &path, &source, modified) {
            parsed_files.push(parsed);
            files_indexed += 1;
        }
    }

    // Re-resolve all edges when any file changed. This is intentionally deterministic and
    // trades a small O(N) pass for correctness across renamed symbols and imports.
    let changed_paths: BTreeSet<String> = parsed_files
        .iter()
        .map(|parsed| parsed.file.path.clone())
        .collect();
    for parsed in &parsed_files {
        graph.insert_file(
            parsed.file.clone(),
            parsed.symbols.clone(),
            parsed.local_edges.clone(),
        );
    }
    if !changed_paths.is_empty() || removed > 0 || options.force {
        rebuild_cross_edges(root, &mut graph)?;
    }

    graph.updated_unix_ms = now_unix_ms();
    storage::save(root, &graph)?;
    Ok(IndexStats {
        files_scanned,
        files_indexed,
        files_skipped_unchanged,
        files_removed: removed,
        symbols: graph.symbols.len(),
        edges: graph.edges.len(),
        bytes_scanned,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

fn rebuild_cross_edges(root: &Path, graph: &mut GraphIndex) -> Result<()> {
    graph.edges.retain(|edge| {
        matches!(edge.kind, crate::model::EdgeKind::Contains | crate::model::EdgeKind::RelatedDecision)
    });
    let mut parsed_files: Vec<ParsedFile> = Vec::new();
    for file in graph.files.values() {
        let path = root.join(&file.path);
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        if is_probably_binary(&bytes) {
            continue;
        }
        let source = String::from_utf8_lossy(&bytes);
        if let Some(parsed) = parse_source(&file.path, &path, &source, file.modified_unix_ms) {
            parsed_files.push(parsed);
        }
    }
    graph.edges.extend(resolve_cross_file_edges(&parsed_files));
    Ok(())
}

pub fn watch(root: &Path, options: &IndexOptions, interval: Duration) -> Result<()> {
    let mut last_fingerprint = tree_fingerprint(root, &read_ignore_patterns(root), options)?;
    eprintln!(
        "watching {} every {} ms; press Ctrl-C to stop",
        root.display(),
        interval.as_millis()
    );
    loop {
        thread::sleep(interval);
        let fingerprint = tree_fingerprint(root, &read_ignore_patterns(root), options)?;
        if fingerprint != last_fingerprint {
            let stats = build(root, options)?;
            eprintln!(
                "updated: {} file(s), {} symbol(s), {} edge(s), {} ms",
                stats.files_indexed, stats.symbols, stats.edges, stats.elapsed_ms
            );
            last_fingerprint = fingerprint;
        }
    }
}

fn tree_fingerprint(root: &Path, patterns: &[String], options: &IndexOptions) -> Result<u64> {
    let files = collect_source_files(root, patterns, options)?;
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for path in files {
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0_u128, |duration| duration.as_millis());
        let relative = normalized_relative(root, &path)?;
        for byte in relative.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= metadata.len();
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        hash ^= modified as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Ok(hash)
}

fn collect_source_files(
    root: &Path,
    patterns: &[String],
    options: &IndexOptions,
) -> Result<Vec<PathBuf>> {
    let canonical_root = fs::canonicalize(root)?;
    let mut output = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut visited_directories = BTreeSet::new();
    visited_directories.insert(canonical_root.clone());
    let mut seen_files = BTreeSet::new();
    while let Some(directory) = stack.pop() {
        let mut entries: Vec<_> = fs::read_dir(&directory)?.filter_map(|entry| entry.ok()).collect();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };
            let relative = match normalized_relative(root, &path) {
                Ok(relative) => relative,
                Err(_) => continue,
            };
            if patterns.iter().any(|pattern| path_matches_pattern(&relative, pattern)) {
                continue;
            }

            if file_type.is_symlink() {
                if !options.follow_symlinks {
                    continue;
                }
                let Ok(canonical_target) = fs::canonicalize(&path) else {
                    continue;
                };
                if !canonical_target.starts_with(&canonical_root) {
                    continue;
                }
                let Ok(metadata) = fs::metadata(&path) else {
                    continue;
                };
                if metadata.is_dir() {
                    if visited_directories.insert(canonical_target) {
                        stack.push(path);
                    }
                } else if metadata.is_file()
                    && metadata.len() <= options.max_file_bytes
                    && crate::parser::detect_language(&path).is_some()
                    && seen_files.insert(canonical_target)
                {
                    output.push(path);
                }
                continue;
            }

            if file_type.is_dir() {
                let Ok(canonical_directory) = fs::canonicalize(&path) else {
                    continue;
                };
                if visited_directories.insert(canonical_directory) {
                    stack.push(path);
                }
            } else if file_type.is_file()
                && crate::parser::detect_language(&path).is_some()
                && entry.metadata().is_ok_and(|metadata| metadata.len() <= options.max_file_bytes)
            {
                let canonical_file = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
                if seen_files.insert(canonical_file) {
                    output.push(path);
                }
            }
        }
    }
    output.sort();
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn indexes_incrementally() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let root = std::env::temp_dir().join(format!("codespace-indexer-{suffix}"));
        fs::create_dir_all(root.join("src"))
            .unwrap_or_else(|error| panic!("create fixture: {error}"));
        fs::write(root.join("src/lib.rs"), "pub fn hello() {}\n")
            .unwrap_or_else(|error| panic!("write fixture: {error}"));
        let first = build(&root, &IndexOptions::default())
            .unwrap_or_else(|error| panic!("first index: {error}"));
        let second = build(&root, &IndexOptions::default())
            .unwrap_or_else(|error| panic!("second index: {error}"));
        assert_eq!(first.files_indexed, 1);
        assert_eq!(second.files_skipped_unchanged, 1);
        let _ = fs::remove_dir_all(root);
    }
}
