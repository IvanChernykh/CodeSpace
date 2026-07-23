use crate::model::{Error, Result};
use crate::util::{json_escape, now_unix_ms, stable_id};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillPermission {
    ReadFiles,
    WriteFiles,
    ExecuteCommands,
    NetworkAccess,
    GitOperations,
    ModifyIndex,
    ManageWorkspaces,
}

impl SkillPermission {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ReadFiles => "read-files",
            Self::WriteFiles => "write-files",
            Self::ExecuteCommands => "execute-commands",
            Self::NetworkAccess => "network-access",
            Self::GitOperations => "git-operations",
            Self::ModifyIndex => "modify-index",
            Self::ManageWorkspaces => "manage-workspaces",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "read-files" => Some(Self::ReadFiles),
            "write-files" => Some(Self::WriteFiles),
            "execute-commands" => Some(Self::ExecuteCommands),
            "network-access" => Some(Self::NetworkAccess),
            "git-operations" => Some(Self::GitOperations),
            "modify-index" => Some(Self::ModifyIndex),
            "manage-workspaces" => Some(Self::ManageWorkspaces),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SkillManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub homepage: String,
    pub permissions: Vec<SkillPermission>,
    pub entry_point: String,
    pub language: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct InstalledSkill {
    pub id: String,
    pub manifest: SkillManifest,
    pub source: String,
    pub installed_unix_ms: u128,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct SkillRegistry {
    pub skills: BTreeMap<String, InstalledSkill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        let mut registry = Self { skills: BTreeMap::new() };
        registry.register_builtins();
        registry
    }

    fn register_builtins(&mut self) {
        let builtins = vec![
            builtin("code-review", "1.0.0", "Automated code review with graph-aware insights", vec![SkillPermission::ReadFiles, SkillPermission::ModifyIndex]),
            builtin("refactor-trace", "1.0.0", "Trace refactoring impact across the codebase", vec![SkillPermission::ReadFiles]),
            builtin("doc-gen", "1.0.0", "Generate documentation from indexed symbols", vec![SkillPermission::ReadFiles]),
            builtin("test-cov", "1.0.0", "Analyze test coverage gaps using graph edges", vec![SkillPermission::ReadFiles]),
            builtin("dep-audit", "1.0.0", "Audit dependencies and detect unused imports", vec![SkillPermission::ReadFiles]),
        ];
        for skill in builtins {
            self.skills.insert(skill.id.clone(), skill);
        }
    }

    pub fn install(&mut self, manifest: SkillManifest, source: &str) -> Result<&InstalledSkill> {
        let id = stable_id(&["skill", &manifest.name, &manifest.version]).to_string();
        if self.skills.contains_key(&id) {
            return Err(Error::InvalidArgument(format!(
                "skill {} v{} is already installed",
                manifest.name, manifest.version
            )));
        }
        let skill = InstalledSkill {
            id: id.clone(),
            manifest,
            source: source.to_string(),
            installed_unix_ms: now_unix_ms(),
            enabled: true,
        };
        self.skills.insert(id.clone(), skill);
        Ok(self.skills.get(&id).ok_or_else(|| Error::CorruptIndex("skill insertion failed".to_string()))?)
    }

    pub fn uninstall(&mut self, id: &str) -> Result<()> {
        if self.skills.remove(id).is_none() {
            return Err(Error::InvalidArgument(format!("skill not found: {id}")));
        }
        Ok(())
    }

    pub fn enable(&mut self, id: &str) -> Result<()> {
        let skill = self.skills.get_mut(id).ok_or_else(|| {
            Error::InvalidArgument(format!("skill not found: {id}"))
        })?;
        skill.enabled = true;
        Ok(())
    }

    pub fn disable(&mut self, id: &str) -> Result<()> {
        let skill = self.skills.get_mut(id).ok_or_else(|| {
            Error::InvalidArgument(format!("skill not found: {id}"))
        })?;
        skill.enabled = false;
        Ok(())
    }

    pub fn list(&self) -> Vec<&InstalledSkill> {
        self.skills.values().collect()
    }

    pub fn list_enabled(&self) -> Vec<&InstalledSkill> {
        self.skills.values().filter(|s| s.enabled).collect()
    }

    pub fn to_json(&self) -> String {
        let skills_json: Vec<String> = self
            .skills
            .values()
            .map(|s| {
                let perms: Vec<String> = s.manifest.permissions.iter().map(|p| format!("\"{}\"", p.as_str())).collect();
                let tags: Vec<String> = s.manifest.tags.iter().map(|t| format!("\"{}\"", json_escape(t))).collect();
                format!(
                    "{{\"id\":\"{}\",\"name\":\"{}\",\"version\":\"{}\",\"description\":\"{}\",\"author\":\"{}\",\"enabled\":{},\"source\":\"{}\",\"permissions\":[{}],\"tags\":[{}]}}",
                    json_escape(&s.id),
                    json_escape(&s.manifest.name),
                    json_escape(&s.manifest.version),
                    json_escape(&s.manifest.description),
                    json_escape(&s.manifest.author),
                    s.enabled,
                    json_escape(&s.source),
                    perms.join(","),
                    tags.join(",")
                )
            })
            .collect();
        format!("{{\"skills\":[{}]}}", skills_json.join(","))
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn builtin(name: &str, version: &str, description: &str, permissions: Vec<SkillPermission>) -> InstalledSkill {
    InstalledSkill {
        id: stable_id(&["skill", name, version]).to_string(),
        manifest: SkillManifest {
            name: name.to_string(),
            version: version.to_string(),
            description: description.to_string(),
            author: "CodeSpace".to_string(),
            homepage: String::new(),
            permissions,
            entry_point: format!("{name}.rs"),
            language: "rust".to_string(),
            tags: vec!["builtin".to_string()],
        },
        source: "builtin".to_string(),
        installed_unix_ms: 0,
        enabled: true,
    }
}

pub fn skills_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".codespace").join("skills")
}

pub fn load_skill_registry() -> SkillRegistry {
    SkillRegistry::new()
}

pub fn parse_manifest_json(content: &str) -> Result<SkillManifest> {
    let mut name = String::new();
    let mut version = String::new();
    let mut description = String::new();
    let mut author = String::new();
    let mut homepage = String::new();
    let mut entry_point = String::new();
    let mut language = String::new();
    let mut permissions = Vec::new();
    let mut tags = Vec::new();

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
                        "name" => { if let Some((v, n)) = parse_string(content, idx) { name = v; idx = n; } else { idx = skip_value(bytes, idx); } }
                        "version" => { if let Some((v, n)) = parse_string(content, idx) { version = v; idx = n; } else { idx = skip_value(bytes, idx); } }
                        "description" => { if let Some((v, n)) = parse_string(content, idx) { description = v; idx = n; } else { idx = skip_value(bytes, idx); } }
                        "author" => { if let Some((v, n)) = parse_string(content, idx) { author = v; idx = n; } else { idx = skip_value(bytes, idx); } }
                        "homepage" => { if let Some((v, n)) = parse_string(content, idx) { homepage = v; idx = n; } else { idx = skip_value(bytes, idx); } }
                        "entry_point" => { if let Some((v, n)) = parse_string(content, idx) { entry_point = v; idx = n; } else { idx = skip_value(bytes, idx); } }
                        "language" => { if let Some((v, n)) = parse_string(content, idx) { language = v; idx = n; } else { idx = skip_value(bytes, idx); } }
                        "permissions" => { idx = skip_ws(bytes, idx); if bytes.get(idx) == Some(&b'[') { idx += 1; loop { idx = skip_ws(bytes, idx); if bytes.get(idx) == Some(&b']') { idx += 1; break; } if let Some((v, n)) = parse_string(content, idx) { if let Some(p) = SkillPermission::parse(&v) { permissions.push(p); } idx = n; } else { break; } idx = skip_ws(bytes, idx); if bytes.get(idx) == Some(&b',') { idx += 1; } } } else { idx = skip_value(bytes, idx); } }
                        "tags" => { idx = skip_ws(bytes, idx); if bytes.get(idx) == Some(&b'[') { idx += 1; loop { idx = skip_ws(bytes, idx); if bytes.get(idx) == Some(&b']') { idx += 1; break; } if let Some((v, n)) = parse_string(content, idx) { tags.push(v); idx = n; } else { break; } idx = skip_ws(bytes, idx); if bytes.get(idx) == Some(&b',') { idx += 1; } } } else { idx = skip_value(bytes, idx); } }
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

    if name.is_empty() {
        return Err(Error::InvalidArgument("manifest missing required field: name".to_string()));
    }
    if version.is_empty() {
        return Err(Error::InvalidArgument("manifest missing required field: version".to_string()));
    }

    Ok(SkillManifest {
        name,
        version,
        description,
        author,
        homepage,
        permissions,
        entry_point,
        language,
        tags,
    })
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
