#!/usr/bin/env python3
from pathlib import Path

path = Path("scripts/self_test.py")
text = path.read_text(encoding="utf-8")

replacements = [
    (
        '        "site/app.js", "scripts/validate_site.py",\n',
        '        "site/app.js", "scripts/validate_site.py",\n'
        '        "dashboard/package.json", "dashboard/package-lock.json",\n'
        '        "dashboard/tsconfig.json", "dashboard/src/main.ts",\n'
        '        "dashboard/dist/main.js", "dashboard/dist/dashboard.css",\n'
        '        "scripts/validate_dashboard_contract.py", "scripts/smoke_dashboard.py",\n',
        "release structure",
    ),
    (
        '    assert names == ["cse_search", "cse_context", "cse_impact", "cse_history", "cse_read"]\n',
        '    assert names == [\n'
        '        "cse_search",\n'
        '        "cse_context",\n'
        '        "cse_impact",\n'
        '        "cse_history",\n'
        '        "cse_read",\n'
        '        "cse_chat",\n'
        '        "cse_task_add",\n'
        '        "cse_task_list",\n'
        '        "cse_task_remove",\n'
        '        "cse_task_status",\n'
        '        "cse_github_status",\n'
        '        "cse_github_issues",\n'
        '    ]\n',
        "MCP tool catalog",
    ),
    (
        '    assert \'"2025-11-25"\' in source and \'"2025-06-18"\' in source\n',
        '    for version in ["2025-11-25", "2025-06-18", "2024-11-05"]:\n'
        '        assert f\'"{version}"\' in source\n',
        "MCP protocol versions",
    ),
    (
        '        "# CodeSpace 1.0 self-test report", "", f"Status: **{report[\'status\'].upper()}**", "",\n',
        '        "# CodeSpace 2.0 self-test report", "", f"Status: **{report[\'status\'].upper()}**", "",\n',
        "self-test report heading",
    ),
]

for old, new, label in replacements:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    text = text.replace(old, new, 1)

path.write_text(text, encoding="utf-8")
