use codespace::context::{build_context, ContextOptions};
use codespace::impact;
use codespace::indexer::{build, IndexOptions};
use codespace::memory::{history, remember, RememberInput};
use codespace::model::SymbolKind;
use codespace::search::find_symbols;
use codespace::secret::redact_secrets;
use codespace::storage;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_project(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let root = std::env::temp_dir().join(format!("codespace-{name}-{suffix}"));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|error| panic!("create temporary project: {error}"));
    root
}

fn write_project(root: &Path) {
    fs::write(
        root.join("src/auth.rs"),
        r#"/// Authenticate without logging credentials.
pub fn authenticate(user: &str, password: &str) -> bool {
    !user.is_empty() && !password.is_empty()
}

pub fn login(user: &str, password: &str) -> bool {
    authenticate(user, password)
}
"#,
    )
    .unwrap_or_else(|error| panic!("write auth fixture: {error}"));
    fs::write(
        root.join("src/api.rs"),
        r#"use crate::auth::login;
pub fn login_endpoint(user: &str, password: &str) -> bool {
    login(user, password)
}
"#,
    )
    .unwrap_or_else(|error| panic!("write api fixture: {error}"));
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("run git {args:?}: {error}"));
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn indexes_searches_compacts_and_persists_decisions() {
    let root = temp_project("library");
    write_project(&root);

    let first = build(&root, &IndexOptions::default())
        .unwrap_or_else(|error| panic!("first index: {error}"));
    let second = build(&root, &IndexOptions::default())
        .unwrap_or_else(|error| panic!("second index: {error}"));
    assert_eq!(first.files_indexed, 2);
    assert_eq!(second.files_skipped_unchanged, 2);

    let mut graph = storage::load(&root).unwrap_or_else(|error| panic!("load index: {error}"));
    let hits = find_symbols(&graph, "login", Some(SymbolKind::Function), 10);
    assert!(!hits.is_empty());

    let bundle = build_context(
        &root,
        &graph,
        "login authentication failure",
        &ContextOptions {
            max_tokens: 500,
            max_items: 4,
            ..ContextOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("build context: {error}"));
    assert!(!bundle.items.is_empty());
    assert!(bundle.estimated_tokens <= 500);

    let id = remember(
        &mut graph,
        RememberInput {
            file: "src/auth.rs".to_string(),
            symbol: "login".to_string(),
            session: "integration-test".to_string(),
            agent: "test".to_string(),
            summary: "Keep authentication local".to_string(),
            rationale: "Avoid credential leakage".to_string(),
            tags: vec!["security".to_string()],
        },
    );
    storage::save(&root, &graph).unwrap_or_else(|error| panic!("save decision: {error}"));
    let reloaded = storage::load(&root).unwrap_or_else(|error| panic!("reload decision: {error}"));
    assert!(reloaded.decisions.contains_key(&id));
    assert_eq!(history(&reloaded, "security", 10).len(), 1);

    let redacted = redact_secrets("OPENAI_API_KEY=sk-proj-1234567890abcdefgh\npassword: correct-horse-battery");
    assert!(redacted.redactions >= 2);
    assert!(!redacted.content.contains("1234567890"));
    assert!(!redacted.content.contains("correct-horse"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn maps_git_changes_to_reverse_callers() {
    if Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let root = temp_project("impact");
    write_project(&root);
    git(&root, &["init", "-q"]);
    git(&root, &["config", "user.email", "test@example.invalid"]);
    git(&root, &["config", "user.name", "CodeSpace Test"]);
    git(&root, &["add", "src"]);
    git(&root, &["commit", "-qm", "baseline"]);
    build(&root, &IndexOptions::default())
        .unwrap_or_else(|error| panic!("index baseline: {error}"));

    let auth = root.join("src/auth.rs");
    let source = fs::read_to_string(&auth).unwrap_or_else(|error| panic!("read auth: {error}"));
    fs::write(&auth, source.replace("!user.is_empty()", "user.len() >= 2"))
        .unwrap_or_else(|error| panic!("modify auth: {error}"));
    git(&root, &["add", "src"]);
    git(&root, &["commit", "-qm", "change authentication"]);
    build(&root, &IndexOptions::default())
        .unwrap_or_else(|error| panic!("index changed revision: {error}"));

    let graph = storage::load(&root).unwrap_or_else(|error| panic!("load impact graph: {error}"));
    let report = impact::analyze(&root, &graph, "HEAD~1", "HEAD", 3)
        .unwrap_or_else(|error| panic!("impact analysis: {error}"));
    assert!(report.changed_files.iter().any(|path| path == "src/auth.rs"));
    assert!(report.changed_symbols.iter().any(|node| node.symbol.contains("authenticate")));
    assert!(report.affected.iter().any(|node| node.symbol.contains("login")));

    let _ = fs::remove_dir_all(root);
}
