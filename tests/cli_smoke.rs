use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_project() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let root = std::env::temp_dir().join(format!("codespace-cli-{suffix}"));
    fs::create_dir_all(root.join("src"))
        .unwrap_or_else(|error| panic!("create CLI project: {error}"));
    fs::write(
        root.join("src/lib.rs"),
        "pub fn authenticate(user: &str) -> bool { !user.is_empty() }\n",
    )
    .unwrap_or_else(|error| panic!("write CLI fixture: {error}"));
    root
}

fn run_cs(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cse"))
        .args(args)
        .arg("--path")
        .arg(root)
        .output()
        .unwrap_or_else(|error| panic!("run cse {args:?}: {error}"))
}

#[test]
fn executes_core_cli_and_mcp_lifecycle() {
    let root = temp_project();

    let init = run_cs(&root, &["init"]);
    assert!(init.status.success(), "{}", String::from_utf8_lossy(&init.stderr));

    let find = run_cs(&root, &["find", "authenticate", "--format", "json"]);
    assert!(find.status.success());
    assert!(String::from_utf8_lossy(&find.stdout).contains("authenticate"));

    let context = run_cs(
        &root,
        &["context", "--query", "authentication", "--format", "json", "--max-tokens", "400"],
    );
    assert!(context.status.success());
    assert!(String::from_utf8_lossy(&context.stdout).contains("estimated_tokens"));

    let remember = run_cs(
        &root,
        &["remember", "--summary", "Do not log credentials", "--file", "src/lib.rs", "--tags", "security"],
    );
    assert!(remember.status.success());
    let history = run_cs(&root, &["history", "security", "--format", "json"]);
    assert!(history.status.success());
    assert!(String::from_utf8_lossy(&history.stdout).contains("Do not log credentials"));

    let export_path = root.join("graph.dot");
    let export = Command::new(env!("CARGO_BIN_EXE_cse"))
        .args(["export", "--format", "graphviz", "--output"])
        .arg(&export_path)
        .arg("--path")
        .arg(&root)
        .output()
        .unwrap_or_else(|error| panic!("export graph: {error}"));
    assert!(export.status.success());
    assert!(fs::read_to_string(&export_path)
        .unwrap_or_else(|error| panic!("read graph export: {error}"))
        .starts_with("digraph codespace"));

    let mut child = Command::new(env!("CARGO_BIN_EXE_cse"))
        .args(["serve", "--mcp", "--path"])
        .arg(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("start MCP server: {error}"));
    {
        let stdin = child.stdin.as_mut().unwrap_or_else(|| panic!("MCP stdin unavailable"));
        writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-11-25","capabilities":{{}},"clientInfo":{{"name":"test","version":"1"}}}}}}"#)
            .unwrap_or_else(|error| panic!("write MCP initialize: {error}"));
        writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{{}}}}"#)
            .unwrap_or_else(|error| panic!("write MCP tools/list: {error}"));
    }
    drop(child.stdin.take());
    let output = child
        .wait_with_output()
        .unwrap_or_else(|error| panic!("wait for MCP server: {error}"));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("2025-11-25"));
    assert!(stdout.contains("cse_context"));
    assert!(stdout.contains("cse_read"));

    let _ = fs::remove_dir_all(root);
}
