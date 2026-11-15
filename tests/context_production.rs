use codespace::context::{ContextOptions, build_context, render_json, render_markdown};
use codespace::model::GraphIndex;
use codespace::parser::parse_source;
use codespace::secret::redact_secrets;
use codespace::util::{estimate_tokens, stable_hash};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    graph: GraphIndex,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "codespace-production-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap_or_else(|error| panic!("fixture: {error}"));
        Self {
            graph: GraphIndex::empty(root.to_string_lossy().into_owned(), 0),
            root,
        }
    }
    fn add(&mut self, path: &str, source: &str) {
        let absolute = self.root.join(path);
        if let Some(parent) = absolute.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|error| panic!("fixture directory: {error}"));
        }
        fs::write(&absolute, source).unwrap_or_else(|error| panic!("fixture source: {error}"));
        let parsed = parse_source(path, &absolute, source, 0)
            .unwrap_or_else(|| panic!("supported fixture language"));
        self.graph
            .insert_file(parsed.file, parsed.symbols, parsed.local_edges);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn canonical_prompt_budget_counts_every_character_and_provenance_tracks_truncation() {
    let mut fixture = Fixture::new();
    let mut source = "pub fn authentication() {\n".to_string();
    for index in 0..100 {
        source.push_str(&format!("    let value_{index} = \"hello  world\";\n"));
    }
    source.push_str("}\n");
    fixture.add("src/auth.rs", &source);
    for budget in [256, 512, 1200] {
        let bundle = build_context(
            &fixture.root,
            &fixture.graph,
            "authentication",
            &ContextOptions {
                max_tokens: budget,
                neighbor_lines: 0,
                ..ContextOptions::default()
            },
        )
        .unwrap_or_else(|error| panic!("context: {error}"));
        let actual = estimate_tokens(&render_markdown(&bundle));
        assert_eq!(bundle.estimated_tokens, actual);
        assert!(actual <= budget, "{actual} > {budget}");
        let item = &bundle.items[0];
        assert!(item.truncated);
        assert!(
            item.content.contains("hello  world"),
            "literal whitespace must survive retrieval"
        );
        assert_eq!(item.line_end, item.content.lines().count());
        assert_eq!(
            item.content_hash,
            format!("{:016x}", stable_hash(item.content.as_bytes()))
        );
        assert_eq!(
            item.source_hash,
            format!("{:016x}", stable_hash(source.as_bytes()))
        );
        assert_eq!(bundle.metrics.baseline_tokens, estimate_tokens(&source));
    }
}

#[test]
fn stale_source_and_cells_are_omitted_instead_of_misattributed() {
    let mut fixture = Fixture::new();
    fixture.add("src/auth.rs", "pub fn authentication() { old_action(); }\n");
    fixture.add("CODESPACE.md", "<!-- cse:cell\nid: authentication\nstatus: accepted\n-->\nAuthentication validates sessions.\n<!-- /cse:cell -->\n");
    fs::write(
        fixture.root.join("src/auth.rs"),
        "pub fn unrelated() { dangerous_action(); }\n",
    )
    .unwrap_or_else(|error| panic!("change: {error}"));
    fs::write(fixture.root.join("CODESPACE.md"), "stale markdown\n")
        .unwrap_or_else(|error| panic!("change: {error}"));
    let bundle = build_context(
        &fixture.root,
        &fixture.graph,
        "authentication",
        &ContextOptions::default(),
    )
    .unwrap_or_else(|error| panic!("context: {error}"));
    assert!(bundle.items.is_empty() && bundle.cells.is_empty());
    assert!(
        bundle
            .warnings
            .iter()
            .any(|warning| warning.contains("changed since indexing"))
    );
    assert!(!render_markdown(&bundle).contains("dangerous_action"));
}

#[test]
fn source_size_limit_is_enforced_again_at_retrieval() {
    let mut fixture = Fixture::new();
    fixture.add("src/auth.rs", "pub fn authentication() {}\n");
    let huge = format!("pub fn authentication() {{}}\n{}", "x".repeat(1_048_577));
    fs::write(fixture.root.join("src/auth.rs"), &huge)
        .unwrap_or_else(|error| panic!("change: {error}"));
    // Even a corrupt/refreshed record cannot bypass the bounded read.
    let id = fixture.graph.files_by_path["src/auth.rs"];
    if let Some(file) = fixture.graph.files.get_mut(&id) {
        file.hash = stable_hash(huge.as_bytes());
    }
    let bundle = build_context(
        &fixture.root,
        &fixture.graph,
        "authentication",
        &ContextOptions::default(),
    )
    .unwrap_or_else(|error| panic!("context: {error}"));
    assert!(bundle.items.is_empty());
    assert!(
        bundle
            .warnings
            .iter()
            .any(|warning| warning.contains("read limit"))
    );
}

#[test]
fn corrupted_index_cannot_read_outside_repository() {
    let mut fixture = Fixture::new();
    fixture.add("src/auth.rs", "pub fn authentication() {}\n");
    let id = fixture.graph.files_by_path["src/auth.rs"];
    if let Some(file) = fixture.graph.files.get_mut(&id) {
        file.path = "../outside.rs".to_string();
    }
    let bundle = build_context(
        &fixture.root,
        &fixture.graph,
        "authentication",
        &ContextOptions::default(),
    )
    .unwrap_or_else(|error| panic!("context: {error}"));
    assert!(bundle.items.is_empty());
    assert!(
        bundle
            .warnings
            .iter()
            .any(|warning| warning.contains("unsafe repository-relative path"))
    );
}

#[test]
fn irrelevant_cells_do_not_override_exact_code_and_linked_files_do_not_inflate_savings() {
    let mut fixture = Fixture::new();
    fixture.add("src/auth.rs", "pub fn authentication() { validate(); }\n");
    let brain = "<!-- cse:cell\nid: design.unrelated\nstatus: accepted\npriority: critical\n-->\nPurple rounded dashboard cards.\n<!-- /cse:cell -->\n\n<!-- cse:cell\nid: auth.contract\nstatus: accepted\ntags: authentication\npaths: huge.bin\n-->\nAuthentication validates sessions.\n<!-- /cse:cell -->\n";
    fixture.add("CODESPACE.md", brain);
    fs::write(fixture.root.join("huge.bin"), vec![0; 500_000])
        .unwrap_or_else(|error| panic!("evidence: {error}"));
    let bundle = build_context(
        &fixture.root,
        &fixture.graph,
        "authentication",
        &ContextOptions {
            max_tokens: 2000,
            ..ContextOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("context: {error}"));
    assert_eq!(bundle.items[0].symbol, "authentication");
    assert_eq!(bundle.cells.len(), 1);
    assert_eq!(bundle.cells[0].id, "auth.contract");
    assert!(
        bundle.source_bytes < 2000,
        "unused linked evidence must not inflate baseline"
    );
    assert_eq!(
        bundle.metrics.baseline_tokens,
        estimate_tokens(brain) + estimate_tokens("pub fn authentication() { validate(); }\n")
    );
}

#[test]
fn multiline_rust_function_retrieval_includes_body() {
    let mut fixture = Fixture::new();
    let source = "pub fn authentication(\n    user: &str,\n) -> bool {\n    validate(user)\n}\n\npub fn next() {}\n";
    fixture.add("src/auth.rs", source);
    let bundle = build_context(
        &fixture.root,
        &fixture.graph,
        "authentication",
        &ContextOptions {
            max_items: 1,
            neighbor_lines: 0,
            ..ContextOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("context: {error}"));
    assert_eq!(bundle.items[0].line_end, 5);
    assert!(bundle.items[0].content.contains("validate(user)"));
    assert!(!bundle.items[0].content.contains("pub fn next"));
}

#[test]
fn redaction_handles_short_decoys_and_preserves_private_key_coordinates() {
    let source = "sk-test then sk-proj-1234567890abcdef\n-----BEGIN PRIVATE KEY-----\nsecret-line-one\nsecret-line-two\n-----END PRIVATE KEY-----\npub fn authentication() {}";
    let redacted = redact_secrets(source);
    assert!(!redacted.content.contains("1234567890abcdef"));
    assert!(!redacted.content.contains("secret-line"));
    assert_eq!(redacted.content.lines().count(), source.lines().count());
    assert_eq!(
        redacted.content.lines().nth(5),
        Some("pub fn authentication() {}")
    );
}

#[test]
fn secret_cell_metadata_is_redacted_and_pack_id_is_stable() {
    let mut fixture = Fixture::new();
    fixture.add("CODESPACE.md", "<!-- cse:cell\nid: auth.contract\ntitle: authentication sk-proj-1234567890abcdef\nstatus: accepted\n-->\nAuthentication validates sessions.\n<!-- /cse:cell -->\n");
    let first = build_context(
        &fixture.root,
        &fixture.graph,
        "authentication",
        &ContextOptions::default(),
    )
    .unwrap_or_else(|error| panic!("context: {error}"));
    let second = build_context(
        &fixture.root,
        &fixture.graph,
        "authentication",
        &ContextOptions::default(),
    )
    .unwrap_or_else(|error| panic!("context: {error}"));
    assert!(!render_json(&first).contains("1234567890abcdef"));
    assert_eq!(first.pack_id, second.pack_id);
}

#[test]
fn impossible_budgets_and_unbounded_queries_return_explicit_errors() {
    let fixture = Fixture::new();
    assert!(
        build_context(
            &fixture.root,
            &fixture.graph,
            "authentication",
            &ContextOptions {
                max_tokens: 10,
                ..ContextOptions::default()
            }
        )
        .is_err()
    );
    assert!(
        build_context(
            &fixture.root,
            &fixture.graph,
            &"x".repeat(2049),
            &ContextOptions::default()
        )
        .is_err()
    );
}

#[test]
fn single_item_budget_can_return_a_whole_knowledge_cell() {
    let mut fixture = Fixture::new();
    fixture.add("CODESPACE.md", "<!-- cse:cell\nid: authentication\nstatus: accepted\n-->\nAuthentication validates sessions.\n<!-- /cse:cell -->\n");
    let bundle = build_context(
        &fixture.root,
        &fixture.graph,
        "authentication",
        &ContextOptions {
            max_items: 1,
            ..ContextOptions::default()
        },
    )
    .unwrap_or_else(|error| panic!("context: {error}"));
    assert_eq!(bundle.cells.len(), 1);
    assert!(bundle.items.is_empty());
}

#[test]
fn longer_markdown_fences_keep_nested_examples_out_of_live_context() {
    let mut fixture = Fixture::new();
    fixture.add("GUIDE.md", "````markdown\n```\n<!-- cse:cell\nid: authentication.example\nstatus: accepted\n-->\nAuthentication example only.\n<!-- /cse:cell -->\n```\n````\n");
    let bundle = build_context(
        &fixture.root,
        &fixture.graph,
        "authentication",
        &ContextOptions::default(),
    )
    .unwrap_or_else(|error| panic!("context: {error}"));
    assert!(bundle.cells.is_empty());
}

#[cfg(unix)]
#[test]
fn symlink_replacement_after_indexing_cannot_escape_repository() {
    let mut inside = Fixture::new();
    let mut outside = Fixture::new();
    let source = "pub fn authentication() {}\n";
    inside.add("auth.rs", source);
    outside.add("auth.rs", source);
    fs::remove_file(inside.root.join("auth.rs")).unwrap_or_else(|error| panic!("replace: {error}"));
    std::os::unix::fs::symlink(outside.root.join("auth.rs"), inside.root.join("auth.rs"))
        .unwrap_or_else(|error| panic!("link: {error}"));
    let bundle = build_context(
        &inside.root,
        &inside.graph,
        "authentication",
        &ContextOptions::default(),
    )
    .unwrap_or_else(|error| panic!("context: {error}"));
    assert!(bundle.items.is_empty());
    assert!(
        bundle
            .warnings
            .iter()
            .any(|warning| warning.contains("escapes"))
    );
}
