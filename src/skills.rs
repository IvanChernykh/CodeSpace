use crate::model::{Error, Result};
use crate::util::{json_escape, now_unix_ms, stable_id};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
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
    removed: BTreeSet<String>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            skills: BTreeMap::new(),
            removed: BTreeSet::new(),
        };
        registry.register_builtins();
        registry
    }

    fn register_builtins(&mut self) {
        let read = || vec![SkillPermission::ReadFiles];
        let read_index = || vec![SkillPermission::ReadFiles, SkillPermission::ModifyIndex];
        let builtins = [
            builtin(
                "code-review",
                "2.0.0",
                "Graph-aware review of correctness, maintainability, and architectural drift",
                read_index(),
                &["builtin", "engineering", "review"],
            ),
            builtin(
                "refactor-trace",
                "2.0.0",
                "Plan refactors and trace their effect across files and symbols",
                read(),
                &["builtin", "engineering", "refactor"],
            ),
            builtin(
                "doc-gen",
                "1.0.0",
                "Compatibility alias for repository-grounded documentation generation",
                read(),
                &["builtin", "compatibility", "docs"],
            ),
            builtin(
                "test-cov",
                "1.0.0",
                "Compatibility alias for graph-based test coverage analysis",
                read(),
                &["builtin", "compatibility", "testing"],
            ),
            builtin(
                "dep-audit",
                "1.0.0",
                "Compatibility alias for dependency and unused import auditing",
                read(),
                &["builtin", "compatibility", "dependencies"],
            ),
            builtin(
                "system-design",
                "2.0.0",
                "Review boundaries, data flow, failure modes, and scalability tradeoffs",
                read(),
                &["builtin", "engineering", "architecture"],
            ),
            builtin(
                "api-design",
                "2.0.0",
                "Review API contracts, compatibility, validation, and error semantics",
                read(),
                &["builtin", "engineering", "api"],
            ),
            builtin(
                "performance-review",
                "2.0.0",
                "Identify likely hot paths, allocation pressure, and algorithmic risks",
                read(),
                &["builtin", "engineering", "performance"],
            ),
            builtin(
                "incident-debug",
                "2.0.0",
                "Structure production debugging from symptoms, evidence, and dependency paths",
                read(),
                &["builtin", "engineering", "debugging"],
            ),
            builtin(
                "security-audit",
                "2.0.0",
                "Review trust boundaries, secret handling, injection risks, and unsafe defaults",
                read(),
                &["builtin", "security", "audit"],
            ),
            builtin(
                "threat-model",
                "2.0.0",
                "Build a repository-grounded threat model with assets, actors, and mitigations",
                read(),
                &["builtin", "security", "threat-model"],
            ),
            builtin(
                "dependency-audit",
                "2.0.0",
                "Inspect dependency usage, supply-chain exposure, and stale integrations",
                read(),
                &["builtin", "security", "dependencies"],
            ),
            builtin(
                "ui-ux-review",
                "2.0.0",
                "Review hierarchy, interaction consistency, empty states, and workflow friction",
                read(),
                &["builtin", "design", "ui", "ux"],
            ),
            builtin(
                "accessibility-review",
                "2.0.0",
                "Check keyboard flow, semantics, contrast, and assistive-technology affordances",
                read(),
                &["builtin", "design", "accessibility"],
            ),
            builtin(
                "design-system",
                "2.0.0",
                "Extract and validate interface tokens, components, and interaction patterns",
                read(),
                &["builtin", "design", "system"],
            ),
            builtin(
                "test-gap-analysis",
                "2.0.0",
                "Find high-risk code paths without direct or indirect test coverage",
                read(),
                &["builtin", "testing", "coverage"],
            ),
            builtin(
                "documentation",
                "2.0.0",
                "Generate and verify architecture and API documentation from indexed code",
                read(),
                &["builtin", "docs", "documentation"],
            ),
            builtin(
                "database-review",
                "2.0.0",
                "Review persistence boundaries, migrations, consistency, and query risks",
                read(),
                &["builtin", "engineering", "database"],
            ),
        ];
        for skill in builtins {
            self.skills.insert(skill.id.clone(), skill);
        }
    }

    pub fn install(&mut self, manifest: SkillManifest, source: &str) -> Result<&InstalledSkill> {
        validate_source(source)?;
        let id = stable_id(&["skill", &manifest.name, &manifest.version]).to_string();
        if self.skills.contains_key(&id) {
            return Err(Error::InvalidArgument(format!(
                "skill {} v{} is already installed",
                manifest.name, manifest.version
            )));
        }
        self.removed.remove(&id);
        let skill = InstalledSkill {
            id: id.clone(),
            manifest,
            source: source.to_string(),
            installed_unix_ms: now_unix_ms(),
            enabled: false,
        };
        self.skills.insert(id.clone(), skill);
        self.skills
            .get(&id)
            .ok_or_else(|| Error::CorruptIndex("skill insertion failed".to_string()))
    }

    pub fn uninstall(&mut self, id: &str) -> Result<()> {
        if self.skills.remove(id).is_none() {
            return Err(Error::InvalidArgument(format!("skill not found: {id}")));
        }
        self.removed.insert(id.to_string());
        Ok(())
    }

    pub fn enable(&mut self, id: &str) -> Result<()> {
        let skill = self
            .skills
            .get_mut(id)
            .ok_or_else(|| Error::InvalidArgument(format!("skill not found: {id}")))?;
        skill.enabled = true;
        Ok(())
    }

    pub fn disable(&mut self, id: &str) -> Result<()> {
        let skill = self
            .skills
            .get_mut(id)
            .ok_or_else(|| Error::InvalidArgument(format!("skill not found: {id}")))?;
        skill.enabled = false;
        Ok(())
    }

    pub fn list(&self) -> Vec<&InstalledSkill> {
        self.skills.values().collect()
    }

    pub fn list_enabled(&self) -> Vec<&InstalledSkill> {
        self.skills.values().filter(|skill| skill.enabled).collect()
    }

    pub fn to_json(&self) -> String {
        let skills_json: Vec<String> = self
            .skills
            .values()
            .map(|skill| {
                let permissions: Vec<String> = skill
                    .manifest
                    .permissions
                    .iter()
                    .map(|permission| format!("\"{}\"", permission.as_str()))
                    .collect();
                let tags: Vec<String> = skill
                    .manifest
                    .tags
                    .iter()
                    .map(|tag| format!("\"{}\"", json_escape(tag)))
                    .collect();
                format!(
                    "{{\"id\":\"{}\",\"name\":\"{}\",\"version\":\"{}\",\"description\":\"{}\",\"author\":\"{}\",\"enabled\":{},\"source\":\"{}\",\"permissions\":[{}],\"tags\":[{}]}}",
                    json_escape(&skill.id),
                    json_escape(&skill.manifest.name),
                    json_escape(&skill.manifest.version),
                    json_escape(&skill.manifest.description),
                    json_escape(&skill.manifest.author),
                    skill.enabled,
                    json_escape(&skill.source),
                    permissions.join(","),
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

fn builtin(
    name: &str,
    version: &str,
    description: &str,
    permissions: Vec<SkillPermission>,
    tags: &[&str],
) -> InstalledSkill {
    InstalledSkill {
        id: stable_id(&["skill", name, version]).to_string(),
        manifest: SkillManifest {
            name: name.to_string(),
            version: version.to_string(),
            description: description.to_string(),
            author: "CodeSpace".to_string(),
            homepage: String::new(),
            permissions,
            entry_point: format!("{name}.md"),
            language: "declarative".to_string(),
            tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
        },
        source: "builtin".to_string(),
        installed_unix_ms: 0,
        enabled: true,
    }
}

fn validate_source(source: &str) -> Result<()> {
    if source == "builtin" || source.starts_with("file:") {
        return Ok(());
    }
    if source.starts_with("https://github.com/") && source.contains("@") {
        return Ok(());
    }
    Err(Error::InvalidArgument(
        "external skills must use a pinned GitHub source such as https://github.com/owner/repo@commit"
            .to_string(),
    ))
}

pub fn skills_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".codespace").join("skills")
}

fn state_path() -> PathBuf {
    skills_dir().join("state.tsv")
}

pub fn load_skill_registry() -> SkillRegistry {
    let mut registry = SkillRegistry::new();
    let Ok(content) = fs::read_to_string(state_path()) else {
        return registry;
    };
    for line in content.lines() {
        let mut fields = line.split('\t');
        match fields.next() {
            Some("skill") => {
                let id = fields.next().unwrap_or_default();
                let enabled = fields.next().unwrap_or("1") == "1";
                if let Some(skill) = registry.skills.get_mut(id) {
                    skill.enabled = enabled;
                }
            }
            Some("removed") => {
                let id = fields.next().unwrap_or_default();
                registry.skills.remove(id);
                registry.removed.insert(id.to_string());
            }
            _ => {}
        }
    }
    registry
}

pub fn save_skill_registry(registry: &SkillRegistry) -> Result<()> {
    fs::create_dir_all(skills_dir())?;
    let mut output = String::new();
    for skill in registry.skills.values() {
        output.push_str(&format!(
            "skill\t{}\t{}\n",
            skill.id,
            if skill.enabled { "1" } else { "0" }
        ));
    }
    for id in &registry.removed {
        output.push_str(&format!("removed\t{id}\n"));
    }
    let path = state_path();
    let temp = path.with_extension("tmp");
    fs::write(&temp, output)?;
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    fs::rename(temp, path)?;
    Ok(())
}

pub fn parse_manifest_json(content: &str) -> Result<SkillManifest> {
    let name = extract_string(content, "name").unwrap_or_default();
    let version = extract_string(content, "version").unwrap_or_default();
    if name.is_empty() {
        return Err(Error::InvalidArgument(
            "manifest missing required field: name".to_string(),
        ));
    }
    if version.is_empty() {
        return Err(Error::InvalidArgument(
            "manifest missing required field: version".to_string(),
        ));
    }
    let permissions = extract_array(content, "permissions")
        .into_iter()
        .filter_map(|value| SkillPermission::parse(&value))
        .collect();
    Ok(SkillManifest {
        name,
        version,
        description: extract_string(content, "description").unwrap_or_default(),
        author: extract_string(content, "author").unwrap_or_default(),
        homepage: extract_string(content, "homepage").unwrap_or_default(),
        permissions,
        entry_point: extract_string(content, "entry_point").unwrap_or_default(),
        language: extract_string(content, "language").unwrap_or_default(),
        tags: extract_array(content, "tags"),
    })
}

fn extract_string(content: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let position = content.find(&needle)?;
    let after = &content[position + needle.len()..];
    let colon = after.find(':')?;
    parse_string(after, colon + 1).map(|(value, _)| value)
}

fn extract_array(content: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\"");
    let Some(position) = content.find(&needle) else {
        return Vec::new();
    };
    let after = &content[position + needle.len()..];
    let Some(colon) = after.find(':') else {
        return Vec::new();
    };
    let bytes = after.as_bytes();
    let mut index = skip_ws(bytes, colon + 1);
    if bytes.get(index) != Some(&b'[') {
        return Vec::new();
    }
    index += 1;
    let mut values = Vec::new();
    loop {
        index = skip_ws(bytes, index);
        if bytes.get(index) == Some(&b']') {
            break;
        }
        let Some((value, next)) = parse_string(after, index) else {
            break;
        };
        values.push(value);
        index = skip_ws(bytes, next);
        if bytes.get(index) == Some(&b',') {
            index += 1;
        }
    }
    values
}

fn parse_string(content: &str, start: usize) -> Option<(String, usize)> {
    let bytes = content.as_bytes();
    let start = skip_ws(bytes, start);
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut output = String::new();
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => return Some((output, index + 1)),
            b'\\' => {
                index += 1;
                match bytes.get(index) {
                    Some(b'"') => output.push('"'),
                    Some(b'\\') => output.push('\\'),
                    Some(b'n') => output.push('\n'),
                    Some(b't') => output.push('\t'),
                    Some(b'r') => output.push('\r'),
                    Some(b'/') => output.push('/'),
                    _ => {}
                }
            }
            _ => {
                let character = content[index..].chars().next()?;
                output.push(character);
                index += character.len_utf8().saturating_sub(1);
            }
        }
        index += 1;
    }
    None
}

fn skip_ws(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}
