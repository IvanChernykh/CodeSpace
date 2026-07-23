use crate::model::{Error, Result};
use crate::util::{index_dir, now_unix_ms, json_escape};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct WorkspaceEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    pub registered_unix_ms: u128,
    pub last_active_unix_ms: u128,
    pub settings: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceRegistry {
    pub workspaces: BTreeMap<String, WorkspaceEntry>,
    pub active_id: Option<String>,
}

impl WorkspaceRegistry {
    pub fn new() -> Self {
        Self {
            workspaces: BTreeMap::new(),
            active_id: None,
        }
    }

    pub fn register(&mut self, path: &Path, name: Option<&str>) -> Result<&WorkspaceEntry> {
        let canonical = fs::canonicalize(path)?;
        if !canonical.is_dir() {
            return Err(Error::InvalidArgument(format!(
                "not a directory: {}", canonical.display()
            )));
        }
        let path_str = canonical.to_string_lossy().to_string();
        let id = crate::util::stable_id(&["workspace", &path_str]);
        let id_str = id.to_string();
        let now = now_unix_ms();
        let ws_name = name.map(|n| n.to_string()).unwrap_or_else(|| {
            canonical
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unnamed".to_string())
        });
        let entry = WorkspaceEntry {
            id: id_str.clone(),
            name: ws_name,
            path: path_str,
            registered_unix_ms: now,
            last_active_unix_ms: now,
            settings: BTreeMap::new(),
        };
        self.workspaces.insert(id_str.clone(), entry);
        self.active_id = Some(id_str.clone());
        Ok(self.workspaces.get(&id_str).ok_or_else(|| Error::CorruptIndex("workspace insertion failed".to_string()))?)
    }

    pub fn remove(&mut self, id: &str) -> Result<()> {
        if self.workspaces.remove(id).is_none() {
            return Err(Error::InvalidArgument(format!("workspace not found: {id}")));
        }
        if self.active_id.as_deref() == Some(id) {
            self.active_id = self.workspaces.keys().next().cloned();
        }
        Ok(())
    }

    pub fn select(&mut self, id: &str) -> Result<()> {
        if !self.workspaces.contains_key(id) {
            return Err(Error::InvalidArgument(format!("workspace not found: {id}")));
        }
        self.active_id = Some(id.to_string());
        if let Some(entry) = self.workspaces.get_mut(id) {
            entry.last_active_unix_ms = now_unix_ms();
        }
        Ok(())
    }

    pub fn active(&self) -> Option<&WorkspaceEntry> {
        self.active_id.as_ref().and_then(|id| self.workspaces.get(id))
    }

    pub fn list(&self) -> Vec<&WorkspaceEntry> {
        self.workspaces.values().collect()
    }

    pub fn to_json(&self) -> String {
        let ws_json: Vec<String> = self
            .workspaces
            .values()
            .map(|ws| {
                format!(
                    "{{\"id\":\"{}\",\"name\":\"{}\",\"path\":\"{}\",\"registered_unix_ms\":{},\"last_active_unix_ms\":{},\"active\":{}}}",
                    json_escape(&ws.id),
                    json_escape(&ws.name),
                    json_escape(&ws.path),
                    ws.registered_unix_ms,
                    ws.last_active_unix_ms,
                    self.active_id.as_deref() == Some(ws.id.as_str())
                )
            })
            .collect();
        format!(
            "{{\"workspaces\":[{}],\"active_id\":{}}}",
            ws_json.join(","),
            self.active_id.as_ref().map(|id| format!("\"{}\"", json_escape(id))).unwrap_or("null".to_string())
        )
    }
}

impl Default for WorkspaceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn global_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".codespace")
}

pub fn global_state_path() -> PathBuf {
    global_dir().join("state.json")
}

pub fn load_global_registry() -> WorkspaceRegistry {
    let path = global_state_path();
    if !path.exists() {
        return WorkspaceRegistry::new();
    }
    match fs::read_to_string(&path) {
        Ok(content) => parse_registry_json(&content),
        Err(_) => WorkspaceRegistry::new(),
    }
}

pub fn save_global_registry(registry: &WorkspaceRegistry) -> Result<()> {
    let dir = global_dir();
    fs::create_dir_all(&dir)?;
    let path = global_state_path();
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, registry.to_json())?;
    #[cfg(not(windows))]
    {
        fs::rename(&temp, &path)?;
    }
    #[cfg(windows)]
    {
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
        fs::rename(&temp, &path)?;
    }
    Ok(())
}

fn parse_registry_json(content: &str) -> WorkspaceRegistry {
    let mut registry = WorkspaceRegistry::new();
    let mut idx = 0;
    let bytes = content.as_bytes();

    while idx < bytes.len() {
        if bytes[idx] == b'"' {
            if let Some((key, end)) = parse_string(content, idx) {
                idx = end;
                idx = skip_ws(bytes, idx);
                if idx < bytes.len() && bytes[idx] == b':' {
                    idx = skip_ws(bytes, idx + 1);
                    if key == "workspaces" {
                        if let Some((items, next)) = parse_array(content, idx) {
                            for item in items {
                                if let Some(ws) = parse_workspace(&item) {
                                    registry.workspaces.insert(ws.id.clone(), ws);
                                }
                            }
                            idx = next;
                        }
                    } else if key == "active_id" {
                        if let Some((val, next)) = parse_string(content, idx) {
                            registry.active_id = Some(val);
                            idx = next;
                        } else {
                            idx = skip_value(bytes, idx);
                        }
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
    registry
}

fn parse_workspace(content: &str) -> Option<WorkspaceEntry> {
    let mut id = String::new();
    let mut name = String::new();
    let mut path = String::new();
    let mut registered = 0_u128;
    let mut last_active = 0_u128;
    let bytes = content.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == b'"' {
            if let Some((key, end)) = parse_string(content, idx) {
                idx = end;
                idx = skip_ws(bytes, idx);
                if idx < bytes.len() && bytes[idx] == b':' {
                    idx = skip_ws(bytes, idx + 1);
                    match key.as_str() {
                        "id" => { if let Some((v, n)) = parse_string(content, idx) { id = v; idx = n; } else { idx = skip_value(bytes, idx); } }
                        "name" => { if let Some((v, n)) = parse_string(content, idx) { name = v; idx = n; } else { idx = skip_value(bytes, idx); } }
                        "path" => { if let Some((v, n)) = parse_string(content, idx) { path = v; idx = n; } else { idx = skip_value(bytes, idx); } }
                        "registered_unix_ms" => { if let Some((v, n)) = parse_number(content, idx) { registered = v; idx = n; } else { idx = skip_value(bytes, idx); } }
                        "last_active_unix_ms" => { if let Some((v, n)) = parse_number(content, idx) { last_active = v; idx = n; } else { idx = skip_value(bytes, idx); } }
                        _ => { idx = skip_value(bytes, idx); }
                    }
                }
            } else {
                idx += 1;
            }
        } else {
            idx += 1;
        }
    }
    if !path.is_empty() {
        Some(WorkspaceEntry {
            id,
            name,
            path,
            registered_unix_ms: registered,
            last_active_unix_ms: last_active,
            settings: BTreeMap::new(),
        })
    } else {
        None
    }
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

fn parse_number(content: &str, start: usize) -> Option<(u128, usize)> {
    let bytes = content.as_bytes();
    let mut idx = start;
    let s = idx;
    if idx < bytes.len() && bytes[idx] == b'-' { idx += 1; }
    while idx < bytes.len() && bytes[idx].is_ascii_digit() { idx += 1; }
    content[s..idx].parse().ok().map(|v| (v, idx))
}

fn parse_array(content: &str, start: usize) -> Option<(Vec<String>, usize)> {
    let bytes = content.as_bytes();
    if bytes.get(start) != Some(&b'[') {
        return None;
    }
    let mut items = Vec::new();
    let mut idx = start + 1;
    let mut depth = 0;
    let mut item_start = idx;
    while idx < bytes.len() {
        match bytes[idx] {
            b'{' => {
                if depth == 0 { item_start = idx; }
                depth += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    items.push(content[item_start..=idx].to_string());
                }
            }
            b']' if depth == 0 => return Some((items, idx + 1)),
            b'"' => {
                if let Some((_, end)) = parse_string(content, idx) {
                    idx = end - 1;
                }
            }
            _ => {}
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
