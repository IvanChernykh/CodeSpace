use codespace::application::{ActionContext, ActionParams, ActionRegistry, OutputFormat};
use codespace::indexer::{build, IndexOptions};
use codespace::model::{Edge, EdgeKind, PrecisionTier};
use codespace::skills::{SkillPermission, SkillRegistry};
use codespace::storage;
use codespace::workspace::{WorkspaceEntry, WorkspaceRegistry};
use codespace::events::{Event, EventType};
use codespace::settings::Settings;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_project(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let root = std::env::temp_dir().join(format!("codespace-v2-{name}-{suffix}"));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|error| panic!("create temporary project: {error}"));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn greet(name: &str) -> String { format!(\"hello {name}\") }\n",
    )
    .unwrap_or_else(|error| panic!("write fixture: {error}"));
    root
}

#[test]
fn action_registry_lists_and_executes_actions() {
    let registry = ActionRegistry::new();
    let actions = registry.list();
    assert!(actions.len() >= 10);
    assert!(actions.iter().any(|a| a.name == "search"));
    assert!(actions.iter().any(|a| a.name == "context"));
    assert!(actions.iter().any(|a| a.name == "stats"));
}

#[test]
fn action_registry_finds_by_alias() {
    let registry = ActionRegistry::new();
    assert!(registry.find("find").is_some());
    assert!(registry.find("search").is_some());
    assert!(registry.find("status").is_some());
    assert!(registry.find("nonexistent").is_none());
}

#[test]
fn action_search_returns_results() {
    let root = temp_project("action-search");
    build(&root, &IndexOptions::default())
        .unwrap_or_else(|error| panic!("index: {error}"));
    let graph = storage::load(&root).unwrap_or_else(|error| panic!("load: {error}"));
    let ctx = ActionContext {
        root: root.clone(),
        graph,
        format: OutputFormat::Json,
    };
    let registry = ActionRegistry::new();
    let mut params = ActionParams::default();
    params.positional.push("greet".to_string());
    let result = registry.execute("search", &ctx, &params)
        .unwrap_or_else(|error| panic!("search action: {error}"));
    assert!(result.stdout.contains("greet"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn action_stats_returns_json() {
    let root = temp_project("action-stats");
    build(&root, &IndexOptions::default())
        .unwrap_or_else(|error| panic!("index: {error}"));
    let graph = storage::load(&root).unwrap_or_else(|error| panic!("load: {error}"));
    let ctx = ActionContext {
        root: root.clone(),
        graph,
        format: OutputFormat::Json,
    };
    let registry = ActionRegistry::new();
    let result = registry.execute("stats", &ctx, &ActionParams::default())
        .unwrap_or_else(|error| panic!("stats action: {error}"));
    assert!(result.stdout.contains("files"));
    assert!(result.stdout.contains("index_revision"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn edge_has_precision_and_evidence() {
    let edge = Edge {
        from: 1,
        to: 2,
        kind: EdgeKind::Calls,
        confidence_milli: 900,
        precision: PrecisionTier::Heuristic,
        evidence: "call: greet".to_string(),
    };
    assert_eq!(edge.precision.as_str(), "heuristic");
    assert_eq!(edge.evidence, "call: greet");
}

#[test]
fn precision_tier_parses_aliases() {
    assert_eq!(PrecisionTier::parse("exact"), Some(PrecisionTier::Exact));
    assert_eq!(PrecisionTier::parse("compiler"), Some(PrecisionTier::Exact));
    assert_eq!(PrecisionTier::parse("lsp"), Some(PrecisionTier::Exact));
    assert_eq!(PrecisionTier::parse("parser"), Some(PrecisionTier::Parser));
    assert_eq!(PrecisionTier::parse("heuristic"), Some(PrecisionTier::Heuristic));
    assert_eq!(PrecisionTier::parse("inferred"), Some(PrecisionTier::Inferred));
    assert_eq!(PrecisionTier::parse("unknown"), None);
}

#[test]
fn new_edge_kinds_parse_correctly() {
    assert_eq!(EdgeKind::parse("extends"), Some(EdgeKind::Extends));
    assert_eq!(EdgeKind::parse("test-covers"), Some(EdgeKind::TestCovers));
    assert_eq!(EdgeKind::parse("configures"), Some(EdgeKind::Configures));
    assert_eq!(EdgeKind::parse("generated-from"), Some(EdgeKind::GeneratedFrom));
    assert_eq!(EdgeKind::parse("depends-on"), Some(EdgeKind::DependsOn));
}

#[test]
fn workspace_registry_register_and_select() {
    let root = temp_project("ws-register");
    let mut registry = WorkspaceRegistry::new();
    let ws = registry.register(&root, Some("test-ws"))
        .unwrap_or_else(|error| panic!("register: {error}"));
    assert_eq!(ws.name, "test-ws");
    assert!(registry.active().is_some());
    assert_eq!(registry.active().unwrap_or_else(|e| panic!("active: {e:?}")).name, "test-ws");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_registry_remove_updates_active() {
    let root1 = temp_project("ws-rm-1");
    let root2 = temp_project("ws-rm-2");
    let mut registry = WorkspaceRegistry::new();
    let ws1 = registry.register(&root1, None).unwrap_or_else(|e| panic!("register1: {e}"));
    let ws2 = registry.register(&root2, None).unwrap_or_else(|e| panic!("register2: {e}"));
    assert_eq!(registry.active().unwrap_or_else(|e| panic!("active: {e:?}")).id, ws2.id);
    registry.remove(&ws2.id).unwrap_or_else(|e| panic!("remove: {e}"));
    assert_eq!(registry.active().unwrap_or_else(|e| panic!("active2: {e:?}")).id, ws1.id);
    let _ = fs::remove_dir_all(root1);
    let _ = fs::remove_dir_all(root2);
}

#[test]
fn skill_registry_has_builtins() {
    let registry = SkillRegistry::new();
    let skills = registry.list();
    assert!(skills.len() >= 5);
    assert!(skills.iter().any(|s| s.manifest.name == "code-review"));
    assert!(skills.iter().any(|s| s.manifest.name == "refactor-trace"));
    assert!(skills.iter().any(|s| s.manifest.name == "doc-gen"));
}

#[test]
fn skill_registry_enable_disable() {
    let mut registry = SkillRegistry::new();
    let skill = registry.list().first().unwrap_or_else(|| panic!("no skills"));
    let id = skill.id.clone();
    registry.disable(&id).unwrap_or_else(|e| panic!("disable: {e}"));
    assert!(!registry.skills.get(&id).unwrap_or_else(|| panic!("skill missing")).enabled);
    registry.enable(&id).unwrap_or_else(|e| panic!("enable: {e}"));
    assert!(registry.skills.get(&id).unwrap_or_else(|| panic!("skill missing")).enabled);
}

#[test]
fn skill_permissions_parse() {
    assert_eq!(SkillPermission::parse("read-files"), Some(SkillPermission::ReadFiles));
    assert_eq!(SkillPermission::parse("write-files"), Some(SkillPermission::WriteFiles));
    assert_eq!(SkillPermission::parse("execute-commands"), Some(SkillPermission::ExecuteCommands));
    assert_eq!(SkillPermission::parse("network-access"), Some(SkillPermission::NetworkAccess));
    assert_eq!(SkillPermission::parse("git-operations"), Some(SkillPermission::GitOperations));
    assert_eq!(SkillPermission::parse("modify-index"), Some(SkillPermission::ModifyIndex));
    assert_eq!(SkillPermission::parse("manage-workspaces"), Some(SkillPermission::ManageWorkspaces));
    assert_eq!(SkillPermission::parse("unknown"), None);
}

#[test]
fn event_serializes_to_json() {
    let event = Event::new(EventType::IndexUpdated, "ws-123", 42)
        .with_data("files", "10")
        .with_data("symbols", "150");
    let json = event.to_json();
    assert!(json.contains("index.updated"));
    assert!(json.contains("ws-123"));
    assert!(json.contains("\"state_version\":42"));
    assert!(json.contains("\"files\":\"10\""));
}

#[test]
fn event_type_parses_roundtrip() {
    for variant in [
        EventType::IndexUpdated,
        EventType::IndexStale,
        EventType::DecisionAdded,
        EventType::WorkspaceRegistered,
        EventType::WorkspaceRemoved,
        EventType::WorkspaceSelected,
        EventType::SettingsChanged,
        EventType::ServerStarted,
        EventType::ServerStopping,
        EventType::SkillInstalled,
        EventType::SkillRemoved,
        EventType::McpServerStarted,
        EventType::McpServerStopped,
    ] {
        let s = variant.as_str();
        assert_eq!(EventType::parse(s), Some(variant));
    }
}

#[test]
fn settings_merge_and_chain() {
    let mut global = Settings::new();
    global.set("theme", "dark");
    global.set("language", "en");

    let mut workspace = Settings::new();
    workspace.set("language", "ru");

    let mut session = Settings::new();
    session.set("font_size", "14");

    let mut chain = codespace::settings::SettingsChain::new();
    chain.global = global;
    chain.workspace = workspace;
    chain.session = session;

    assert_eq!(chain.get("theme"), Some("dark".to_string()));
    assert_eq!(chain.get("language"), Some("ru".to_string()));
    assert_eq!(chain.get("font_size"), Some("14".to_string()));
    assert_eq!(chain.get_or("missing", "default"), "default");
}

#[test]
fn index_revision_increments_on_build() {
    let root = temp_project("revision");
    build(&root, &IndexOptions::default())
        .unwrap_or_else(|error| panic!("first build: {error}"));
    let graph1 = storage::load(&root).unwrap_or_else(|error| panic!("load1: {error}"));
    let rev1 = graph1.index_revision;
    assert!(rev1 > 0);
    build(&root, &IndexOptions::default())
        .unwrap_or_else(|error| panic!("second build: {error}"));
    let graph2 = storage::load(&root).unwrap_or_else(|error| panic!("load2: {error}"));
    assert!(graph2.index_revision > rev1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn storage_v1_migration_sets_schema_v2() {
    let root = temp_project("migration");
    let dir = root.join(".codespace");
    fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("mkdir: {error}"));
    let v1_content = "CODESPACE\t1\nMETA\t1\tsrc/lib.rs\t1000\t2000\n";
    fs::write(dir.join("index.csf"), v1_content).unwrap_or_else(|error| panic!("write: {error}"));
    let graph = storage::load(&root).unwrap_or_else(|error| panic!("load v1: {error}"));
    assert_eq!(graph.schema_version, 2);
    let _ = fs::remove_dir_all(root);
}
