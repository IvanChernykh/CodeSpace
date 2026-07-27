#!/usr/bin/env python3
from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


server = Path("src/server.rs")
replace_once(
    server,
    "            let state_guard = state.lock().unwrap_or_else(|error| error.into_inner());\n            state_guard.events.publish(&Event::new(EventType::SettingsChanged, \"\", 0).with_data(\"key\", &key));",
    "            let mut state_guard = state.lock().unwrap_or_else(|error| error.into_inner());\n            state_guard.events.publish(&Event::new(EventType::SettingsChanged, \"\", 0).with_data(\"key\", &key));",
    "post-apply settings event guard",
)

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

tasks = Path("src/tasks.rs")
replace_once(
    tasks,
    '        self.tasks.get(&id).expect("inserted task must exist")\n',
    '        self.tasks\n            .get(&id)\n            .unwrap_or_else(|| unreachable!("inserted task must exist"))\n',
    "post-apply task insertion invariant",
)
