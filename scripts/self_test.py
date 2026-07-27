#!/usr/bin/env python3
"""End-to-end behavioral self-test for CodeSpace source releases.

The harness uses only Python's standard library, but it executes the real `cse`
binary. It validates packaging, dashboard assets, secret-safe context, Git-aware
impact, decision memory, task persistence, and the MCP initialize/tools lifecycle.
"""

from __future__ import annotations

import json
import os
import queue
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import tomllib
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

EXPECTED_MCP_TOOLS = [
    "cse_search",
    "cse_context",
    "cse_impact",
    "cse_history",
    "cse_read",
    "cse_chat",
    "cse_task_add",
    "cse_task_list",
    "cse_task_remove",
    "cse_task_status",
    "cse_github_status",
    "cse_github_issues",
]


@dataclass
class Check:
    name: str
    status: str
    detail: str
    duration_ms: int


class Suite:
    def __init__(self) -> None:
        self.checks: list[Check] = []

    def run(self, name: str, function) -> None:
        started = time.perf_counter()
        try:
            detail = function()
            status = "pass"
        except Exception as error:  # test harness must report every failed invariant
            detail = f"{type(error).__name__}: {error}"
            status = "fail"
        duration_ms = int((time.perf_counter() - started) * 1000)
        self.checks.append(Check(name, status, str(detail), duration_ms))
        print(f"[{status.upper():4}] {name}: {detail}")

    @property
    def failures(self) -> list[Check]:
        return [check for check in self.checks if check.status != "pass"]


def command(
    arguments: list[str],
    *,
    cwd: Path,
    input_text: str | None = None,
    expected: int = 0,
    timeout: int = 30,
) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        arguments,
        cwd=cwd,
        input=input_text,
        text=True,
        capture_output=True,
        timeout=timeout,
        check=False,
    )
    if result.returncode != expected:
        raise AssertionError(
            f"command failed ({result.returncode}): {' '.join(arguments)}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result


def resolve_binary(root: Path) -> Path:
    override = os.environ.get("CSE_BIN")
    candidates = [
        Path(override) if override else None,
        root / "target" / "debug" / ("cse.exe" if os.name == "nt" else "cse"),
        root / "target" / "release" / ("cse.exe" if os.name == "nt" else "cse"),
    ]
    for candidate in candidates:
        if candidate is not None and candidate.is_file():
            return candidate.resolve()
    cargo = shutil.which("cargo")
    if cargo is None:
        raise AssertionError("cse binary is missing and cargo is unavailable")
    command([cargo, "build", "--locked"], cwd=root, timeout=180)
    binary = root / "target" / "debug" / ("cse.exe" if os.name == "nt" else "cse")
    if not binary.is_file():
        raise AssertionError(f"cargo build did not produce {binary}")
    return binary.resolve()


def parse_json_output(result: subprocess.CompletedProcess[str]) -> Any:
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise AssertionError(f"invalid JSON output: {result.stdout}") from error


def check_manifest(root: Path) -> str:
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    package = manifest["package"]
    assert package["name"] == "codespace-cse"
    assert package["version"] == "2.0.0"
    assert package["edition"] == "2024"
    assert package["rust-version"] == "1.85"
    assert manifest["bin"][0]["name"] == "cse"
    includes = set(package["include"])
    for required in {
        "dashboard/src/**",
        "dashboard/dist/**",
        "dashboard/package.json",
        "dashboard/package-lock.json",
        "dashboard/tsconfig.json",
    }:
        assert required in includes, f"Cargo package excludes {required}"
    return "manifest and packaged dashboard contract are consistent"


def check_structure(root: Path) -> str:
    required = [
        "src/main.rs",
        "src/lib.rs",
        "src/server.rs",
        "src/dashboard.rs",
        "src/mcp.rs",
        "src/mcp_manager.rs",
        "src/skills.rs",
        "src/github_integration.rs",
        "dashboard/package.json",
        "dashboard/package-lock.json",
        "dashboard/tsconfig.json",
        "dashboard/src/main.ts",
        "dashboard/src/api.ts",
        "dashboard/src/graph-view.ts",
        "dashboard/dist/main.js",
        "dashboard/dist/dashboard.css",
        "scripts/validate_dashboard_contract.py",
        "scripts/smoke_dashboard.py",
        ".github/workflows/ci.yml",
        ".github/workflows/pages.yml",
        "site/index.html",
    ]
    missing = [path for path in required if not (root / path).is_file()]
    assert not missing, f"missing release assets: {missing}"
    dashboard = (root / "src/dashboard.rs").read_text(encoding="utf-8")
    assert dashboard.count("<script") == 1, "dashboard exposes multiple runtimes"
    assert 'src="/assets/main.js"' in dashboard
    return f"{len(required)} required release assets and one dashboard runtime present"


def check_static_security(root: Path) -> str:
    manifest = (root / "Cargo.toml").read_text(encoding="utf-8")
    assert 'unsafe_code = "forbid"' in manifest
    violations: list[str] = []
    for path in sorted((root / "src").glob("*.rs")):
        source = path.read_text(encoding="utf-8")
        for line_number, line in enumerate(source.splitlines(), 1):
            if re.search(r"\bunsafe\s*\{", line):
                violations.append(f"{path.name}:{line_number}: unsafe block")
            if ".unwrap(" in line or ".expect(" in line:
                violations.append(f"{path.name}:{line_number}: unwrap/expect")
    assert not violations, "; ".join(violations)
    github = (root / "src/github_integration.rs").read_text(encoding="utf-8")
    assert "gh auth login" in github and "gh api" in github
    save_section = github.split("pub fn save_config", 1)[1].split("fn parse_config", 1)[0]
    assert '\"token\"' not in save_section, "GitHub credential is persisted in config"
    return "unsafe and panic-prone calls blocked; GitHub uses gh credential/TLS transport"


def initialize_fixture(root: Path, binary: Path) -> dict[str, Any]:
    (root / "src").mkdir(parents=True)
    (root / "src" / "auth.rs").write_text(
        """/// Authenticate a local user without exposing credentials.
pub fn login(user: &str) -> bool {
    !user.trim().is_empty()
}

pub const API_TOKEN: &str = "ghp_1234567890abcdefghijklmnop";
""",
        encoding="utf-8",
    )
    (root / "src" / "lib.rs").write_text(
        "mod auth;\npub fn allow(user: &str) -> bool { auth::login(user) }\n",
        encoding="utf-8",
    )
    (root / "README.md").write_text("# Fixture\n", encoding="utf-8")
    command(["git", "init", "-q"], cwd=root)
    command(["git", "config", "user.email", "selftest@example.invalid"], cwd=root)
    command(["git", "config", "user.name", "CodeSpace Self-Test"], cwd=root)
    command(["git", "add", "."], cwd=root)
    command(["git", "commit", "-qm", "baseline"], cwd=root)
    command([str(binary), "init", "--path", str(root), "--force"], cwd=root)
    graph = parse_json_output(
        command([str(binary), "graph", "--path", str(root), "--format", "json"], cwd=root)
    )
    assert graph["files"] and graph["symbols"]
    return graph


def check_search_context(root: Path, binary: Path, metrics: dict[str, Any]) -> str:
    search = parse_json_output(
        command(
            [str(binary), "find", "login", "--path", str(root), "--format", "json"],
            cwd=root,
        )
    )
    assert search, "search returned no login symbol"
    context_result = command(
        [
            str(binary),
            "context",
            "--path",
            str(root),
            "--query",
            "login authentication token",
            "--format",
            "json",
            "--max-tokens",
            "900",
        ],
        cwd=root,
    )
    context = parse_json_output(context_result)
    serialized = json.dumps(context, ensure_ascii=False)
    assert "ghp_1234567890" not in serialized, "secret leaked through context"
    redactions = sum(int(item.get("redactions", 0)) for item in context.get("items", []))
    assert redactions >= 1, "fixture secret was not reported as redacted"
    metrics.update(
        {
            "search_hits": len(search),
            "context_items": len(context.get("items", [])),
            "context_redactions": redactions,
            "estimated_tokens": context.get("estimated_tokens", 0),
        }
    )
    return f"{len(search)} search hits; {redactions} secret redaction(s) in real context"


def check_impact_memory_tasks(root: Path, binary: Path, metrics: dict[str, Any]) -> str:
    auth = root / "src" / "auth.rs"
    auth.write_text(
        auth.read_text(encoding="utf-8").replace(
            "!user.trim().is_empty()", "user.trim().len() >= 2"
        ),
        encoding="utf-8",
    )
    command(["git", "add", "."], cwd=root)
    command(["git", "commit", "-qm", "tighten login"], cwd=root)
    command([str(binary), "update", "--path", str(root), "--force"], cwd=root)
    impact = parse_json_output(
        command(
            [
                str(binary),
                "impact",
                "--path",
                str(root),
                "--from",
                "HEAD~1",
                "--to",
                "HEAD",
                "--format",
                "json",
            ],
            cwd=root,
        )
    )
    assert "src/auth.rs" in impact.get("changed_files", [])

    command(
        [
            str(binary),
            "remember",
            "--path",
            str(root),
            "--summary",
            "Require two-character user names",
            "--file",
            "src/auth.rs",
            "--symbol",
            "login",
            "--rationale",
            "Reject accidental one-character identities",
        ],
        cwd=root,
    )
    history = parse_json_output(
        command(
            [str(binary), "history", "login", "--path", str(root), "--format", "json"],
            cwd=root,
        )
    )
    assert history and history[0]["summary"] == "Require two-character user names"

    command(
        [
            str(binary),
            "task",
            "add",
            "--path",
            str(root),
            "--title",
            "Review auth impact",
            "--priority",
            "high",
            "--tags",
            "security,review",
        ],
        cwd=root,
    )
    tasks = parse_json_output(
        command(
            [str(binary), "task", "list", "--path", str(root), "--format", "json"],
            cwd=root,
        )
    )
    assert any(task["title"] == "Review auth impact" for task in tasks["tasks"])
    metrics.update(
        {
            "impact_changed_files": impact.get("changed_files", []),
            "history_records": len(history),
            "tasks": len(tasks["tasks"]),
        }
    )
    return "Git impact, decision memory, and task persistence passed"


def read_json_line(
    lines: "queue.Queue[str | None]", expected_id: int, timeout: float = 8.0
) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        remaining = max(0.05, deadline - time.monotonic())
        try:
            line = lines.get(timeout=remaining)
        except queue.Empty as error:
            raise AssertionError(f"timed out waiting for MCP response {expected_id}") from error
        if line is None:
            raise AssertionError("MCP server closed stdout")
        message = json.loads(line)
        if message.get("id") == expected_id:
            return message
    raise AssertionError(f"timed out waiting for MCP response {expected_id}")


def check_mcp(root: Path, binary: Path, metrics: dict[str, Any]) -> str:
    process = subprocess.Popen(
        [str(binary), "serve", "--mcp", "--path", str(root)],
        cwd=root,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1,
    )
    if process.stdin is None or process.stdout is None:
        raise AssertionError("failed to open MCP stdio")
    lines: "queue.Queue[str | None]" = queue.Queue()

    def drain() -> None:
        assert process.stdout is not None
        for line in process.stdout:
            lines.put(line.strip())
        lines.put(None)

    threading.Thread(target=drain, daemon=True).start()
    try:
        initialize = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "codespace-self-test", "version": "2.0.0"},
            },
        }
        process.stdin.write(json.dumps(initialize) + "\n")
        process.stdin.flush()
        initialized = read_json_line(lines, 1)
        assert initialized["result"]["protocolVersion"] == "2025-06-18"
        process.stdin.write(
            json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"
        )
        process.stdin.write(
            json.dumps({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}})
            + "\n"
        )
        process.stdin.flush()
        catalog = read_json_line(lines, 2)
        names = [tool["name"] for tool in catalog["result"]["tools"]]
        assert names == EXPECTED_MCP_TOOLS, names
        for tool in catalog["result"]["tools"]:
            assert tool["description"]
            assert tool["inputSchema"]["type"] == "object"
        metrics["mcp_tools"] = names
        return f"MCP initialize and tools/list passed for {len(names)} tools"
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def write_reports(root: Path, suite: Suite, metrics: dict[str, Any], started: float) -> None:
    artifacts = root / "artifacts"
    artifacts.mkdir(exist_ok=True)
    report = {
        "release": "2.0.0",
        "generated_unix_ms": int(time.time() * 1000),
        "elapsed_ms": int((time.perf_counter() - started) * 1000),
        "status": "pass" if not suite.failures else "fail",
        "checks": [asdict(check) for check in suite.checks],
        "metrics": metrics,
        "validation_boundary": [
            "The harness executes the real debug or release cse binary.",
            "The separate CI runtime smoke test validates the embedded localhost HTTP dashboard.",
            "External Ollama, GitHub account, and third-party MCP servers require explicit integration tests with credentials/services.",
        ],
    }
    (artifacts / "self-test-report.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    rows = [
        "# CodeSpace 2.0 self-test report",
        "",
        f"Status: **{report['status'].upper()}**",
        "",
        "| Check | Status | Evidence | Duration |",
        "|---|---:|---|---:|",
    ]
    for check in suite.checks:
        detail = check.detail.replace("|", "\\|").replace("\n", " ")
        rows.append(f"| {check.name} | {check.status} | {detail} | {check.duration_ms} ms |")
    rows.extend(["", "## Metrics", "", "```json", json.dumps(metrics, ensure_ascii=False, indent=2), "```"])
    (artifacts / "SELF_TEST.md").write_text("\n".join(rows) + "\n", encoding="utf-8")
    (artifacts / "self-context.txt").write_text(
        json.dumps(metrics, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    started = time.perf_counter()
    suite = Suite()
    metrics: dict[str, Any] = {}
    binary = resolve_binary(root)

    suite.run("manifest", lambda: check_manifest(root))
    suite.run("release structure", lambda: check_structure(root))
    suite.run("static security", lambda: check_static_security(root))

    with tempfile.TemporaryDirectory(prefix="codespace-self-test-") as directory:
        fixture = Path(directory)
        suite.run(
            "real indexing",
            lambda: f"indexed {len(initialize_fixture(fixture, binary)['files'])} fixture files",
        )
        suite.run("search/context/redaction", lambda: check_search_context(fixture, binary, metrics))
        suite.run(
            "impact/memory/tasks", lambda: check_impact_memory_tasks(fixture, binary, metrics)
        )
        suite.run("MCP lifecycle", lambda: check_mcp(fixture, binary, metrics))
        suite.run(
            "doctor",
            lambda: command([str(binary), "doctor", "--path", str(fixture)], cwd=fixture).stdout.strip()
            or "doctor completed",
        )

    write_reports(root, suite, metrics, started)
    print(f"\nReport: {root / 'artifacts/self-test-report.json'}")
    return 1 if suite.failures else 0


if __name__ == "__main__":
    sys.exit(main())
