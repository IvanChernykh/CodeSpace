use crate::model::{Error, Result};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const INDEX_DIR: &str = ".codespace";
pub const INDEX_FILE: &str = "index.csf";
pub const CONFIG_FILE: &str = "config";
pub const LOCK_FILE: &str = "write.lock";
pub const DEFAULT_MAX_FILE_BYTES: u64 = 1_048_576;

pub fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

pub fn canonical_root(path: &Path) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path)?;
    if !canonical.is_dir() {
        return Err(Error::InvalidArgument(format!(
            "path is not a directory: {}",
            canonical.display()
        )));
    }
    Ok(canonical)
}

pub fn index_dir(root: &Path) -> PathBuf {
    root.join(INDEX_DIR)
}

pub fn index_path(root: &Path) -> PathBuf {
    index_dir(root).join(INDEX_FILE)
}

pub fn config_path(root: &Path) -> PathBuf {
    index_dir(root).join(CONFIG_FILE)
}

pub fn lock_path(root: &Path) -> PathBuf {
    index_dir(root).join(LOCK_FILE)
}

pub fn normalized_relative(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| Error::InvalidArgument(format!("path escapes project root: {}", path.display())))?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => parts.push(value.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(Error::InvalidArgument(format!(
                    "unsafe relative path: {}",
                    relative.display()
                )));
            }
        }
    }
    Ok(parts.join("/"))
}

pub fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

pub fn stable_id(parts: &[&str]) -> u64 {
    let mut bytes = Vec::new();
    for part in parts {
        bytes.extend_from_slice(part.as_bytes());
        bytes.push(0xff);
    }
    stable_hash(&bytes)
}

pub fn escape_field(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '\t' => output.push_str("\\t"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            _ => output.push(character),
        }
    }
    output
}

pub fn unescape_field(value: &str) -> Result<String> {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        let Some(escaped) = chars.next() else {
            return Err(Error::CorruptIndex("trailing escape in field".to_string()));
        };
        match escaped {
            '\\' => output.push('\\'),
            't' => output.push('\t'),
            'n' => output.push('\n'),
            'r' => output.push('\r'),
            other => {
                output.push('\\');
                output.push(other);
            }
        }
    }
    Ok(output)
}

pub fn split_escaped_tsv(line: &str) -> Result<Vec<String>> {
    line.split('\t').map(unescape_field).collect()
}

pub fn json_escape(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 8);
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                output.push_str(&format!("\\u{:04x}", character as u32));
            }
            _ => output.push(character),
        }
    }
    output
}

pub fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn tokenize(value: &str) -> Vec<String> {
    let mut tokens = BTreeSet::new();
    let mut current = String::new();
    let mut previous_lowercase = false;
    for character in value.chars() {
        if character.is_alphanumeric() || character == '_' {
            if character.is_uppercase() && previous_lowercase && !current.is_empty() {
                if current.len() > 1 {
                    tokens.insert(current.to_ascii_lowercase());
                }
                current.clear();
            }
            previous_lowercase = character.is_lowercase();
            current.push(character);
        } else {
            if current.len() > 1 {
                tokens.insert(current.to_ascii_lowercase());
            }
            current.clear();
            previous_lowercase = false;
        }
    }
    if current.len() > 1 {
        tokens.insert(current.to_ascii_lowercase());
    }
    tokens.into_iter().collect()
}

pub fn estimate_tokens(value: &str) -> usize {
    value.chars().count().div_ceil(4)
}

pub fn is_probably_binary(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    if bytes.iter().take(8192).any(|byte| *byte == 0) {
        return true;
    }
    let sample = &bytes[..bytes.len().min(8192)];
    let suspicious = sample
        .iter()
        .filter(|byte| **byte < 0x09 || (**byte > 0x0d && **byte < 0x20))
        .count();
    suspicious * 100 / sample.len().max(1) > 5
}

pub fn path_matches_pattern(path: &str, pattern: &str) -> bool {
    let normalized = pattern.trim().trim_start_matches("./");
    if normalized.is_empty() || normalized.starts_with('#') {
        return false;
    }
    if normalized.ends_with('/') {
        let prefix = normalized.trim_end_matches('/');
        return path == prefix
            || path.starts_with(&format!("{prefix}/"))
            || path.ends_with(&format!("/{prefix}"))
            || path.contains(&format!("/{prefix}/"));
    }
    if let Some(suffix) = normalized.strip_prefix("*.") {
        return path.ends_with(&format!(".{suffix}"));
    }
    if normalized.contains('*') {
        return wildcard_match(path.as_bytes(), normalized.as_bytes());
    }
    path == normalized || path.ends_with(&format!("/{normalized}"))
}

fn wildcard_match(value: &[u8], pattern: &[u8]) -> bool {
    let (mut value_index, mut pattern_index) = (0, 0);
    let (mut star_index, mut checkpoint) = (None, 0);
    while value_index < value.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == b'?' || pattern[pattern_index] == value[value_index])
        {
            value_index += 1;
            pattern_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star_index = Some(pattern_index);
            checkpoint = value_index;
            pattern_index += 1;
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            checkpoint += 1;
            value_index = checkpoint;
        } else {
            return false;
        }
    }
    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

pub fn read_ignore_patterns(root: &Path) -> Vec<String> {
    let mut patterns = vec![
        ".git/".to_string(),
        ".codespace/".to_string(),
        "target/".to_string(),
        "node_modules/".to_string(),
        "dist/".to_string(),
        "build/".to_string(),
        "vendor/".to_string(),
        "coverage/".to_string(),
        "__pycache__/".to_string(),
        ".venv/".to_string(),
        "venv/".to_string(),
        ".env".to_string(),
        ".env.*".to_string(),
        "credentials.json".to_string(),
        "secrets.json".to_string(),
        "*.pem".to_string(),
        "*.key".to_string(),
        "*.p12".to_string(),
        "*.pfx".to_string(),
    ];
    for filename in [".gitignore", ".codespaceignore"] {
        let path = root.join(filename);
        if let Ok(content) = fs::read_to_string(path) {
            patterns.extend(
                content
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('!'))
                    .map(ToOwned::to_owned),
            );
        }
    }
    patterns
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_nested_ignored_directories() {
        assert!(path_matches_pattern("apps/web/node_modules/pkg/index.js", "node_modules/"));
        assert!(path_matches_pattern("nested/.git/config", ".git/"));
        assert!(!path_matches_pattern("src/node_modules_helper.rs", "node_modules/"));
    }

    #[test]
    fn escapes_round_trip() {
        let original = "a\tb\nc\\d";
        let escaped = escape_field(original);
        let restored = unescape_field(&escaped)
            .unwrap_or_else(|error| panic!("unescape field: {error}"));
        assert_eq!(restored, original);
    }
}
