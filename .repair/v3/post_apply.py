#!/usr/bin/env python3
from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


def replace_all(path: Path, old: str, new: str, expected: int, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{label}: expected {expected} matches, found {count}")
    path.write_text(text.replace(old, new), encoding="utf-8")


server = Path("src/server.rs")
replace_once(
    server,
    "            let state_guard = state.lock().unwrap_or_else(|error| error.into_inner());\n            state_guard.events.publish(&Event::new(EventType::SettingsChanged, \"\", 0).with_data(\"key\", &key));",
    "            let mut state_guard = state.lock().unwrap_or_else(|error| error.into_inner());\n            state_guard.events.publish(&Event::new(EventType::SettingsChanged, \"\", 0).with_data(\"key\", &key));",
    "post-apply settings event guard",
)
replace_once(
    server,
    '    let request = format!(\n        "GET /api/v1/health HTTP/1.1\\r\\nHost: localhost\\r\\nConnection: close\\r\\n\\r\\n"\n    );',
    '    let request = "GET /api/v1/health HTTP/1.1\\r\\nHost: localhost\\r\\nConnection: close\\r\\n\\r\\n".to_string();',
    "server health request allocation",
)
replace_once(server, "    root: &PathBuf,\n", "    root: &Path,\n", "server path argument")
replace_all(server, 'params.get("force").is_some()', 'params.contains_key("force")', 1, "server force flag")
replace_all(server, 'params.get("repair").is_some()', 'params.contains_key("repair")', 1, "server repair flag")
server_text = server.read_text(encoding="utf-8")
server_text = server_text.replace("const MAX_REQUEST_SIZE: usize = 1_048_576;\n", "", 1)
server.write_text(server_text, encoding="utf-8")

skills = Path("src/skills.rs")
replace_once(
    skills,
    '            builtin("refactor-trace", "2.0.0", "Plan refactors and trace their effect across files and symbols", read(), &["builtin", "engineering", "refactor"]),\n',
    '            builtin("refactor-trace", "2.0.0", "Plan refactors and trace their effect across files and symbols", read(), &["builtin", "engineering", "refactor"]),\n'
    '            builtin("doc-gen", "1.0.0", "Compatibility alias for repository-grounded documentation generation", read(), &["builtin", "compatibility", "docs"]),\n'
    '            builtin("test-cov", "1.0.0", "Compatibility alias for graph-based test coverage analysis", read(), &["builtin", "compatibility", "testing"]),\n'
    '            builtin("dep-audit", "1.0.0", "Compatibility alias for dependency and unused import auditing", read(), &["builtin", "compatibility", "dependencies"]),\n',
    "post-apply legacy skill compatibility",
)

replace_once(
    Path("src/ai.rs"),
    "    sessions.sort_by(|a, b| b.1.cmp(&a.1));",
    "    sessions.sort_by_key(|item| std::cmp::Reverse(item.1));",
    "AI session ordering",
)
replace_once(
    Path("src/cli.rs"),
    '    let path = args.value("path").map_or_else(|| PathBuf::from("."), |value| PathBuf::from(value));',
    '    let path = args.value("path").map_or_else(|| PathBuf::from("."), PathBuf::from);',
    "CLI path construction",
)
replace_once(
    Path("src/events.rs"),
    "pub struct EventBus {\n    subscribers: Vec<Box<dyn Fn(&Event) + Send + Sync>>,\n}",
    "type EventSubscriber = Box<dyn Fn(&Event) + Send + Sync>;\n\npub struct EventBus {\n    subscribers: Vec<EventSubscriber>,\n}",
    "event subscriber alias",
)
replace_once(
    Path("src/github_integration.rs"),
    '    let path = format!("/user/repos?per_page=50&sort=updated");',
    '    let path = "/user/repos?per_page=50&sort=updated".to_string();',
    "GitHub repository path",
)
replace_once(
    Path("src/memory.rs"),
    "    decisions.sort_by(|left, right| right.timestamp_unix_ms.cmp(&left.timestamp_unix_ms));",
    "    decisions.sort_by_key(|decision| std::cmp::Reverse(decision.timestamp_unix_ms));",
    "decision ordering",
)
replace_once(
    Path("src/model.rs"),
    "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]\npub enum PrecisionTier {\n    Exact,\n    Parser,",
    "#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]\npub enum PrecisionTier {\n    Exact,\n    #[default]\n    Parser,",
    "precision default derive",
)
replace_once(
    Path("src/model.rs"),
    "impl Default for PrecisionTier {\n    fn default() -> Self {\n        Self::Parser\n    }\n}\n\n",
    "",
    "precision manual default removal",
)
replace_once(
    Path("src/parser.rs"),
    "            .split(|character: char| character.is_whitespace() || character == ':' || character == '*')\n            .filter(|part| !part.is_empty())\n            .next_back()?;",
    "            .split(|character: char| character.is_whitespace() || character == ':' || character == '*')\n            .rfind(|part| !part.is_empty())?;",
    "parser declaration tail",
)
replace_once(
    Path("src/rest.rs"),
    "use std::path::{Path, PathBuf};",
    "use std::path::Path;",
    "REST path import",
)
replace_once(
    Path("src/rest.rs"),
    "fn handle_connection(mut stream: TcpStream, root: &PathBuf) -> Result<()> {",
    "fn handle_connection(mut stream: TcpStream, root: &Path) -> Result<()> {",
    "REST path argument",
)
replace_once(
    Path("src/secret.rs"),
    "        loop {\n            let Some(start) = find_token_prefix(&output, prefix) else {\n                break;\n            };",
    "        while let Some(start) = find_token_prefix(&output, prefix) {",
    "secret token loop",
)
replace_once(
    Path("src/secret.rs"),
    "output[position..].find(|character| character == '=' || character == ':')",
    "output[position..].find(['=', ':'])",
    "secret separator matcher",
)
replace_all(
    Path("src/tasks.rs"),
    '.map_or("null".to_string(), |v| v.to_string())',
    '.map_or_else(|| "null".to_string(), |v| v.to_string())',
    2,
    "task optional timestamps",
)
replace_once(
    Path("src/workspace.rs"),
    '        Ok(self.workspaces.get(&id_str).ok_or_else(|| Error::CorruptIndex("workspace insertion failed".to_string()))?)',
    '        self.workspaces\n            .get(&id_str)\n            .ok_or_else(|| Error::CorruptIndex("workspace insertion failed".to_string()))',
    "workspace insertion return",
)
