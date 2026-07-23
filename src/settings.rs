use crate::model::{Error, Result};
use crate::util::json_escape;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Settings {
    pub values: BTreeMap<String, String>,
}

impl Settings {
    pub fn new() -> Self {
        Self { values: BTreeMap::new() }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn get_or(&self, key: &str, default: &str) -> String {
        self.values.get(key).cloned().unwrap_or_else(|| default.to_string())
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.values.insert(key.to_string(), value.to_string());
    }

    pub fn merge(&mut self, other: &Settings) {
        for (key, value) in &other.values {
            self.values.insert(key.clone(), value.clone());
        }
    }

    pub fn to_json(&self) -> String {
        let pairs: Vec<String> = self
            .values
            .iter()
            .map(|(k, v)| format!("\"{}\":\"{}\"", json_escape(k), json_escape(v)))
            .collect();
        format!("{{{}}}", pairs.join(","))
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SettingsChain {
    pub global: Settings,
    pub workspace: Settings,
    pub session: Settings,
}

impl SettingsChain {
    pub fn new() -> Self {
        Self {
            global: load_global_settings(),
            workspace: Settings::new(),
            session: Settings::new(),
        }
    }

    pub fn effective(&self) -> Settings {
        let mut result = Settings::new();
        result.merge(&self.global);
        result.merge(&self.workspace);
        result.merge(&self.session);
        result
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.session.get(key)
            .or_else(|| self.workspace.get(key))
            .or_else(|| self.global.get(key))
            .map(|s| s.to_string())
    }

    pub fn get_or(&self, key: &str, default: &str) -> String {
        self.get(key).unwrap_or_else(|| default.to_string())
    }
}

impl Default for SettingsChain {
    fn default() -> Self {
        Self::new()
    }
}

pub fn global_settings_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".codespace").join("settings.json")
}

pub fn workspace_settings_path(root: &Path) -> PathBuf {
    crate::util::index_dir(root).join("settings.json")
}

pub fn load_global_settings() -> Settings {
    let path = global_settings_path();
    if let Ok(content) = fs::read_to_string(&path) {
        parse_settings_json(&content)
    } else {
        Settings::new()
    }
}

pub fn load_workspace_settings(root: &Path) -> Settings {
    let path = workspace_settings_path(root);
    if let Ok(content) = fs::read_to_string(&path) {
        parse_settings_json(&content)
    } else {
        Settings::new()
    }
}

pub fn save_global_settings(settings: &Settings) -> Result<()> {
    let path = global_settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, settings.to_json())?;
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    fs::rename(&temp, &path)?;
    Ok(())
}

pub fn save_workspace_settings(root: &Path, settings: &Settings) -> Result<()> {
    let dir = crate::util::index_dir(root);
    fs::create_dir_all(&dir)?;
    let path = workspace_settings_path(root);
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, settings.to_json())?;
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    fs::rename(&temp, &path)?;
    Ok(())
}

fn parse_settings_json(content: &str) -> Settings {
    let mut settings = Settings::new();
    let bytes = content.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == b'"' {
            if let Some((key, end)) = parse_string(content, idx) {
                idx = end;
                idx = skip_ws(bytes, idx);
                if idx < bytes.len() && bytes[idx] == b':' {
                    idx = skip_ws(bytes, idx + 1);
                    if let Some((value, next)) = parse_string(content, idx) {
                        settings.set(&key, &value);
                        idx = next;
                    } else {
                        idx = skip_value(bytes, idx);
                    }
                }
            } else {
                idx += 1;
            }
        } else {
            idx += 1;
        }
    }
    settings
}

fn parse_string(content: &str, start: usize) -> Option<(String, usize)> {
    let bytes = content.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut output = String::new();
    let mut idx = start + 1;
    while idx < bytes.len() {
        match bytes[idx] {
            b'"' => return Some((output, idx + 1)),
            b'\\' => {
                idx += 1;
                match bytes.get(idx) {
                    Some(&b'"') => output.push('"'),
                    Some(&b'\\') => output.push('\\'),
                    Some(&b'/') => output.push('/'),
                    Some(&b'n') => output.push('\n'),
                    Some(&b't') => output.push('\t'),
                    Some(&b'r') => output.push('\r'),
                    _ => {}
                }
            }
            _ => {
                let remaining = &content[idx..];
                if let Some(ch) = remaining.chars().next() {
                    output.push(ch);
                    idx += ch.len_utf8() - 1;
                }
            }
        }
        idx += 1;
    }
    None
}

fn skip_ws(bytes: &[u8], mut idx: usize) -> usize {
    while idx < bytes.len() && matches!(bytes[idx], b' ' | b'\t' | b'\n' | b'\r') {
        idx += 1;
    }
    idx
}

fn skip_value(bytes: &[u8], mut idx: usize) -> usize {
    let mut depth = 0;
    while idx < bytes.len() {
        match bytes[idx] {
            b'{' | b'[' => depth += 1,
            b'}' | b']' => { if depth == 0 { return idx; } depth -= 1; }
            b',' if depth == 0 => return idx,
            b'"' => { idx += 1; while idx < bytes.len() && bytes[idx] != b'"' { if bytes[idx] == b'\\' { idx += 1; } idx += 1; } }
            _ => {}
        }
        idx += 1;
    }
    idx
}
