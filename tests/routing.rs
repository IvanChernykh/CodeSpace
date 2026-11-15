use codespace::routing::{BulkReadOptions, TaskKind, bulk_read, read_source};
use codespace::util::estimate_tokens;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "cse-routing-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap_or_else(|e| panic!("fixture: {e}"));
        Self(root)
    }
    fn write(&self, path: &str, content: &str) {
        fs::write(self.0.join(path), content).unwrap_or_else(|e| panic!("fixture write: {e}"));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn corpus(lines: usize) -> String {
    (1..=lines)
        .map(|line| {
            if line == 299 {
                "pub fn cache_timeout() -> usize { 12 }\n".to_string()
            } else {
                format!("// unrelated source line {line}\n")
            }
        })
        .collect()
}

#[test]
fn large_read_returns_stable_numbered_evidence_with_complete_budget_accounting() {
    let f = Fixture::new();
    f.write("src/large.rs", &corpus(900));
    let options = BulkReadOptions::default();
    let paths = vec!["src/large.rs".to_string()];
    let first =
        bulk_read(&f.0, "cache timeout", &paths, &options).unwrap_or_else(|e| panic!("bulk: {e}"));
    let second =
        bulk_read(&f.0, "cache timeout", &paths, &options).unwrap_or_else(|e| panic!("bulk: {e}"));
    assert_eq!(first.to_json(), second.to_json());
    assert_eq!(first.route, "local-extract");
    assert!(
        first
            .snippets
            .iter()
            .any(|s| s.content.contains("299 | pub fn cache_timeout"))
    );
    assert!(first.metrics.saved_tokens_estimate > 0);
    let json = first.to_json().to_string();
    assert_eq!(first.metrics.coordinator_result_bytes, json.len());
    assert_eq!(
        first.metrics.coordinator_result_tokens_estimate,
        estimate_tokens(&json).max(estimate_tokens(&first.render_plain()))
    );
    assert!(first.metrics.coordinator_result_tokens_estimate <= options.max_tokens);
    assert_eq!(first.metrics.network_bytes, 0);
    assert!(!first.to_json()["worker_invoked"].as_bool().unwrap_or(true));
}

#[test]
fn targeted_reads_preserve_original_lines_through_private_key_redaction() {
    let f = Fixture::new();
    f.write("src/key.rs", "before\n-----BEGIN PRIVATE KEY-----\nvery-secret-key-material\n-----END PRIVATE KEY-----\nafter\nfinal\n");
    let read = read_source(&f.0, "src/key.rs", 3, 3).unwrap_or_else(|e| panic!("read: {e}"));
    assert_eq!(
        (read.line_start, read.line_end, read.total_lines),
        (3, 5, 6)
    );
    assert_eq!(read.redactions, 1);
    assert!(read.content.contains("5 | after"));
    assert!(!read.content.contains("very-secret"));
    assert!(read.truncated);
    assert!(read_source(&f.0, "src/key.rs", 7, 1).is_err());
    assert!(read_source(&f.0, "src/key.rs", 0, 1).is_err());
}

#[test]
fn threshold_risk_and_explicit_ranges_control_routing() {
    let f = Fixture::new();
    f.write("src/small.rs", &corpus(350));
    f.write("src/large.rs", &corpus(351));
    let small = vec!["src/small.rs".to_string()];
    let large = vec!["src/large.rs".to_string()];
    let mut options = BulkReadOptions::default();
    assert_eq!(
        bulk_read(&f.0, "cache", &small, &options)
            .unwrap_or_else(|e| panic!("bulk: {e}"))
            .route,
        "direct"
    );
    options.task_kind = TaskKind::Overview;
    assert_eq!(
        bulk_read(&f.0, "cache", &large, &options)
            .unwrap_or_else(|e| panic!("bulk: {e}"))
            .route,
        "worker-recommended"
    );
    options.task_kind = TaskKind::Security;
    assert_eq!(
        bulk_read(&f.0, "cache", &large, &options)
            .unwrap_or_else(|e| panic!("bulk: {e}"))
            .route,
        "targeted"
    );
    options.task_kind = TaskKind::Inspect;
    options.start_line = Some(298);
    options.max_lines = Some(3);
    let targeted =
        bulk_read(&f.0, "cache", &large, &options).unwrap_or_else(|e| panic!("bulk: {e}"));
    assert_eq!(targeted.route, "targeted");
    assert!(
        targeted
            .snippets
            .iter()
            .all(|s| s.line_start >= 298 && s.line_end <= 300)
    );
}

#[test]
fn secret_files_ignore_rules_binary_and_path_escape_fail_closed() {
    let f = Fixture::new();
    f.write(".codespaceignore", "src/ignored.rs\n");
    for path in [
        "src/.env.production",
        "src/.ENV",
        "src/id_rsa",
        "src/credentials.json",
        "src/ignored.rs",
        "src/key.pem",
    ] {
        f.write(path, "private material");
        assert!(read_source(&f.0, path, 1, 10).is_err(), "must block {path}");
    }
    f.write("src/binary.rs", "abc\0def");
    assert!(read_source(&f.0, "src/binary.rs", 1, 10).is_err());
    assert!(read_source(&f.0, "../outside.rs", 1, 10).is_err());
    assert!(read_source(&f.0, "src/../outside.rs", 1, 10).is_err());
    assert!(read_source(&f.0, &f.0.join("src/binary.rs").to_string_lossy(), 1, 10).is_err());
}

#[cfg(unix)]
#[test]
fn symlink_files_and_directory_components_are_not_followed() {
    let f = Fixture::new();
    f.write("src/source.rs", "public source\n");
    std::os::unix::fs::symlink(f.0.join("src/source.rs"), f.0.join("link.rs"))
        .unwrap_or_else(|e| panic!("symlink: {e}"));
    std::os::unix::fs::symlink(f.0.join("src"), f.0.join("linked"))
        .unwrap_or_else(|e| panic!("symlink: {e}"));
    assert!(read_source(&f.0, "link.rs", 1, 10).is_err());
    assert!(read_source(&f.0, "linked/source.rs", 1, 10).is_err());
}

#[test]
fn unmatched_large_reads_report_previews_not_invented_answers() {
    let f = Fixture::new();
    f.write("src/large.rs", &corpus(400));
    let result = bulk_read(
        &f.0,
        "quantum elephant",
        &["src/large.rs".to_string()],
        &BulkReadOptions::default(),
    )
    .unwrap_or_else(|e| panic!("bulk: {e}"));
    assert!(
        result
            .warnings
            .iter()
            .any(|s| s.contains("No question terms matched"))
    );
    assert!(result.snippets.iter().all(|s| s.score == 0));
}

#[test]
fn unicode_escaping_metadata_and_huge_lines_cannot_exceed_the_output_budget() {
    let f = Fixture::new();
    f.write(
        "src/large.rs",
        &format!("{}\n{}", "таймаут \\\"".repeat(5000), corpus(400)),
    );
    let options = BulkReadOptions {
        max_tokens: 700,
        ..BulkReadOptions::default()
    };
    let result = bulk_read(
        &f.0,
        "таймаут cache",
        &["src/large.rs".to_string()],
        &options,
    )
    .unwrap_or_else(|e| panic!("bulk: {e}"));
    assert!(estimate_tokens(&result.to_json().to_string()) <= options.max_tokens);
    assert!(estimate_tokens(&result.render_plain()) <= options.max_tokens);
    assert!(
        result
            .snippets
            .iter()
            .all(|s| !s.content.contains("таймаут"))
    );
}

#[test]
fn rejects_oversized_input_and_infeasible_metadata_budget() {
    let f = Fixture::new();
    f.write("src/large.rs", &"a".repeat(1_048_577));
    assert!(read_source(&f.0, "src/large.rs", 1, 2).is_err());
    f.write("src/small.rs", "cache line\n");
    let paths = vec!["src/small.rs".to_string()];
    assert!(
        bulk_read(
            &f.0,
            &"cache ".repeat(250),
            &paths,
            &BulkReadOptions {
                max_tokens: 256,
                ..BulkReadOptions::default()
            }
        )
        .is_err()
    );
    assert!(bulk_read(&f.0, "", &paths, &BulkReadOptions::default()).is_err());
}

#[test]
fn rare_terms_and_file_diversity_prevent_generic_json_from_hiding_security_evidence() {
    let f = Fixture::new();
    let generic: String = (0..500)
        .map(|line| format!("    let input_{line} = json_input(request);\n"))
        .collect();
    f.write("src/aaa_mcp.rs", &generic);
    let mut server: Vec<String> = corpus(500).lines().map(str::to_string).collect();
    server[299] = "pub fn validate_origin(request: &Request) -> bool {".to_string();
    server[300] = "    request.headers.authentication.is_valid()".to_string();
    server[301] = "}".to_string();
    f.write("src/zzz_server.rs", &server.join("\n"));
    let options = BulkReadOptions {
        max_tokens: 1400,
        task_kind: TaskKind::Security,
        ..BulkReadOptions::default()
    };
    let result = bulk_read(
        &f.0,
        "request authentication headers origin JSON input validation",
        &[
            "src/aaa_mcp.rs".to_string(),
            "src/zzz_server.rs".to_string(),
        ],
        &options,
    )
    .unwrap_or_else(|e| panic!("bulk: {e}"));
    assert!(result.snippets.iter().any(|s| s.path == "src/aaa_mcp.rs"));
    assert!(
        result
            .snippets
            .iter()
            .any(|s| s.path == "src/zzz_server.rs" && s.content.contains("validate_origin"))
    );
    assert!(result.snippets.iter().all(|s| s.line_end > s.line_start));
    assert!(estimate_tokens(&result.to_json().to_string()) <= options.max_tokens);
}
