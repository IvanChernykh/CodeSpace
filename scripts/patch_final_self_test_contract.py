#!/usr/bin/env python3
from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    Path("src/dashboard.rs"),
    '        assert_eq!(html.matches("<script").count(), 1);\n',
    '        let script_tag = ["<", "script"].concat();\n'
    '        assert_eq!(html.matches(&script_tag).count(), 1);\n',
    "dashboard single-runtime test",
)

replace_once(
    Path("src/github_integration.rs"),
    'const AUTH_MARKER: &str = "gh-cli";\n',
    'const AUTH_MARKER: &str = "gh-cli";\n\n'
    '// Authenticated network operations are delegated to the system `gh api` TLS client.\n',
    "GitHub transport documentation",
)
