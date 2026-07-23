#!/usr/bin/env python3
"""Behavioral and static self-test for CodeSpace source releases.

This harness deliberately uses only Python's standard library. It validates release
structure, Rust lexical balance, MCP tool schemas, clean-room security invariants,
and a reference implementation of indexing/ranking/compaction against the CodeSpace
repository itself. It is not a substitute for `cargo test` or Clippy.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import tomllib
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Iterable

SUPPORTED = {
    ".rs": "rust", ".py": "python", ".pyi": "python", ".js": "javascript",
    ".jsx": "javascript", ".ts": "typescript", ".tsx": "typescript",
    ".go": "go", ".java": "java", ".kt": "kotlin", ".kts": "kotlin",
    ".c": "c", ".h": "c", ".cc": "cpp", ".cpp": "cpp", ".hpp": "cpp",
    ".cs": "csharp", ".swift": "swift", ".php": "php", ".rb": "ruby",
    ".sh": "shell", ".bash": "shell", ".zsh": "shell", ".lua": "lua",
    ".scala": "scala", ".ex": "elixir", ".exs": "elixir", ".dart": "dart",
    ".vue": "vue", ".svelte": "svelte", ".sql": "sql", ".proto": "protobuf",
    ".toml": "toml", ".yaml": "yaml", ".yml": "yaml", ".json": "json",
    ".md": "markdown", ".mdx": "markdown",
}
IGNORED_DIRS = {".git", ".codespace", "target", "node_modules", "dist", "build", "vendor", "coverage", "__pycache__", ".venv", "venv"}
TOKEN_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]{1,}")
CALL_RE = re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(")
CALL_STOP = {"if", "for", "while", "match", "switch", "catch", "return", "sizeof", "typeof", "fn", "function", "def", "class", "struct", "enum", "trait", "interface", "Some", "Ok", "Err", "println", "print", "assert", "assert_eq", "vec"}


@dataclass(frozen=True)
class Symbol:
    id: str
    path: str
    language: str
    name: str
    kind: str
    line_start: int
    line_end: int
    signature: str
    doc: str


@dataclass
class Check:
    name: str
    status: str
    detail: str
    duration_ms: int


class Suite:
    def __init__(self) -> None:
        self.checks: list[Check] = []

    def run(self, name: str, fn) -> None:
        started = time.perf_counter()
        try:
            detail = fn()
            status = "pass"
        except Exception as exc:  # test harness must report every failure
            detail = f"{type(exc).__name__}: {exc}"
            status = "fail"
        elapsed = int((time.perf_counter() - started) * 1000)
        self.checks.append(Check(name, status, str(detail), elapsed))
        print(f"[{status.upper():4}] {name}: {detail}")

    @property
    def failures(self) -> list[Check]:
        return [check for check in self.checks if check.status != "pass"]


def stable_id(*parts: str) -> str:
    digest = hashlib.sha256()
    for part in parts:
        digest.update(part.encode("utf-8"))
        digest.update(b"\xff")
    return digest.hexdigest()[:16]


def source_files(root: Path) -> list[Path]:
    output: list[Path] = []
    for current, dirs, files in os.walk(root):
        dirs[:] = sorted(directory for directory in dirs if directory not in IGNORED_DIRS)
        for filename in sorted(files):
            path = Path(current, filename)
            if path.suffix.lower() in SUPPORTED and path.stat().st_size <= 1_048_576:
                output.append(path)
    return sorted(output)


def strip_rust_comments_and_strings(source: str) -> str:
    """Preserve newlines while replacing comments/string bodies with spaces."""
    out: list[str] = []
    i = 0
    n = len(source)
    block_depth = 0
    while i < n:
        if block_depth:
            if source.startswith("/*", i):
                block_depth += 1
                out.extend("  ")
                i += 2
            elif source.startswith("*/", i):
                block_depth -= 1
                out.extend("  ")
                i += 2
            else:
                out.append("\n" if source[i] == "\n" else " ")
                i += 1
            continue
        if source.startswith("//", i):
            while i < n and source[i] != "\n":
                out.append(" ")
                i += 1
            continue
        if source.startswith("/*", i):
            block_depth = 1
            out.extend("  ")
            i += 2
            continue
        # Rust raw string: r###"..."### or br###"..."###
        raw_match = re.match(r"(?:br|r)(#{0,16})\"", source[i:])
        if raw_match:
            hashes = raw_match.group(1)
            opener_len = raw_match.end()
            out.extend(" " * opener_len)
            i += opener_len
            closer = '"' + hashes
            end = source.find(closer, i)
            if end < 0:
                raise AssertionError("unterminated Rust raw string")
            while i < end + len(closer):
                out.append("\n" if source[i] == "\n" else " ")
                i += 1
            continue
        if source[i] in {'"', "'"}:
            quote = source[i]
            # A single quote can start a lifetime. Treat it as a char only when a closing quote is nearby.
            if quote == "'" and not re.match(r"'(?:\\.|[^\\'\n])'", source[i:]):
                out.append(source[i])
                i += 1
                continue
            out.append(" ")
            i += 1
            escaped = False
            while i < n:
                ch = source[i]
                out.append("\n" if ch == "\n" else " ")
                i += 1
                if escaped:
                    escaped = False
                elif ch == "\\":
                    escaped = True
                elif ch == quote:
                    break
            else:
                raise AssertionError("unterminated Rust quoted literal")
            continue
        out.append(source[i])
        i += 1
    if block_depth:
        raise AssertionError("unterminated Rust block comment")
    return "".join(out)


def assert_balanced_rust(path: Path) -> tuple[int, int]:
    source = path.read_text(encoding="utf-8")
    cleaned = strip_rust_comments_and_strings(source)
    stack: list[tuple[str, int]] = []
    pairs = {')': '(', ']': '[', '}': '{'}
    for index, char in enumerate(cleaned):
        if char in "([{":
            stack.append((char, index))
        elif char in ")]}":
            if not stack or stack[-1][0] != pairs[char]:
                line = cleaned.count("\n", 0, index) + 1
                raise AssertionError(f"unbalanced {char} at {path}:{line}")
            stack.pop()
    if stack:
        char, index = stack[-1]
        line = cleaned.count("\n", 0, index) + 1
        raise AssertionError(f"unclosed {char} at {path}:{line}")
    return len(source.splitlines()), len(TOKEN_RE.findall(cleaned))


def declaration(line: str, language: str) -> tuple[str, str] | None:
    stripped = line.strip()
    if language == "rust":
        cleaned = re.sub(r"^pub(?:\([^)]*\))?\s+", "", stripped)
        match = re.match(r"(?:(?:async|unsafe|const)\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)", cleaned)
        if match:
            return match.group(1), "function"
        for keyword, kind in [("struct", "struct"), ("enum", "enum"), ("trait", "trait"), ("mod", "module"), ("type", "type_alias"), ("const", "constant"), ("static", "variable")]:
            match = re.match(rf"{keyword}\s+([A-Za-z_][A-Za-z0-9_]*)", cleaned)
            if match:
                return match.group(1), kind
        match = re.match(r"impl(?:<[^>]+>)?\s+([A-Za-z_][A-Za-z0-9_]*)", cleaned)
        if match:
            return f"impl_{match.group(1)}", "module"
    elif language == "python":
        match = re.match(r"(?:async\s+)?def\s+([A-Za-z_][A-Za-z0-9_]*)", stripped)
        if match:
            return match.group(1), "function"
        match = re.match(r"class\s+([A-Za-z_][A-Za-z0-9_]*)", stripped)
        if match:
            return match.group(1), "class"
    elif language in {"javascript", "typescript", "vue", "svelte"}:
        cleaned = re.sub(r"^(?:export\s+)?(?:default\s+)?(?:declare\s+)?(?:async\s+)?", "", stripped)
        match = re.match(r"(function|class|interface|enum|type|namespace)\s+([A-Za-z_$][A-Za-z0-9_$]*)", cleaned)
        if match:
            kinds = {"function": "function", "class": "class", "interface": "interface", "enum": "enum", "type": "type_alias", "namespace": "module"}
            return match.group(2), kinds[match.group(1)]
        match = re.match(r"(?:const|let|var)\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*=.*(?:=>|function)", cleaned)
        if match:
            return match.group(1), "function"
    elif language == "go":
        match = re.match(r"func\s+(?:\([^)]*\)\s*)?([A-Za-z_][A-Za-z0-9_]*)", stripped)
        if match:
            return match.group(1), "function"
        match = re.match(r"type\s+([A-Za-z_][A-Za-z0-9_]*)\s+(struct|interface)?", stripped)
        if match:
            return match.group(1), match.group(2) or "type_alias"
    elif language in {"ruby", "shell", "elixir", "lua"}:
        match = re.match(r"(?:def|function|defmodule|class|module)\s+([A-Za-z_][A-Za-z0-9_]*)", stripped)
        if match:
            return match.group(1), "function"
    else:
        match = re.match(r"(?:public\s+|private\s+|protected\s+|internal\s+|static\s+|final\s+|abstract\s+)*(?:class|struct|enum|interface|protocol|record|namespace)\s+([A-Za-z_][A-Za-z0-9_]*)", stripped)
        if match:
            return match.group(1), "type"
    return None


def parse_symbols(root: Path, files: Iterable[Path]) -> tuple[list[Symbol], list[tuple[str, str]]]:
    symbols: list[Symbol] = []
    calls: list[tuple[str, str]] = []
    for path in files:
        rel = path.relative_to(root).as_posix()
        language = SUPPORTED[path.suffix.lower()]
        try:
            source = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        lines = source.splitlines()
        local: list[Symbol] = []
        pending_doc: list[str] = []
        for idx, line in enumerate(lines, 1):
            stripped = line.strip()
            if stripped.startswith(("///", "//!", "# ")):
                pending_doc.append(stripped.lstrip("/#! "))
                continue
            decl = declaration(line, language)
            if decl:
                name, kind = decl
                symbol = Symbol(
                    id=stable_id("symbol", rel, name, kind, str(idx)),
                    path=rel,
                    language=language,
                    name=name,
                    kind=kind,
                    line_start=idx,
                    line_end=min(len(lines), idx + 40),
                    signature=stripped[:500],
                    doc=" ".join(pending_doc),
                )
                pending_doc.clear()
                local.append(symbol)
                symbols.append(symbol)
            elif stripped:
                pending_doc.clear()
            if local:
                owner = local[-1].id
                for call in CALL_RE.findall(line):
                    if call not in CALL_STOP and call != local[-1].name:
                        calls.append((owner, call))
    return symbols, calls


def build_edges(symbols: list[Symbol], calls: list[tuple[str, str]]) -> list[tuple[str, str, str]]:
    by_name: dict[str, list[Symbol]] = {}
    for symbol in symbols:
        by_name.setdefault(symbol.name.lower(), []).append(symbol)
    edges: list[tuple[str, str, str]] = []
    for owner, name in calls:
        candidates = by_name.get(name.lower(), [])
        if len(candidates) == 1 and candidates[0].id != owner:
            edges.append((owner, candidates[0].id, "calls"))
    return sorted(set(edges))


def tokens(value: str) -> set[str]:
    return {match.lower() for match in TOKEN_RE.findall(value)}


def rank(symbols: list[Symbol], edges: list[tuple[str, str, str]], query: str, limit: int = 12) -> list[tuple[int, Symbol]]:
    query_lower = query.lower()
    query_tokens = tokens(query)
    score: dict[str, int] = {}
    by_id = {symbol.id: symbol for symbol in symbols}
    for symbol in symbols:
        haystack = f"{symbol.name} {symbol.path} {symbol.signature} {symbol.doc}".lower()
        value = 0
        if symbol.name.lower() == query_lower:
            value += 10_000
        elif symbol.name.lower().startswith(query_lower):
            value += 6_000
        elif query_lower and query_lower in symbol.name.lower():
            value += 4_000
        if query_lower and query_lower in symbol.path.lower():
            value += 2_000
        if query_lower and query_lower in symbol.signature.lower():
            value += 1_500
        value += 700 * len(query_tokens & tokens(haystack))
        if value:
            score[symbol.id] = value
    for source, target, _ in edges:
        if source in score and target in by_id:
            score[target] = score.get(target, 0) + max(150, score[source] // 8)
        if target in score and source in by_id:
            score[source] = score.get(source, 0) + max(150, score[target] // 8)
    ranked = sorted(((value, by_id[symbol_id]) for symbol_id, value in score.items()), key=lambda item: (-item[0], item[1].path, item[1].line_start))
    return ranked[:limit]


def redact(value: str) -> tuple[str, int]:
    count = 0
    output, private_keys = re.subn(
        r"(?ms)-----BEGIN [^-]*PRIVATE KEY-----.*?-----END [^-]*PRIVATE KEY-----",
        "[REDACTED PRIVATE KEY BLOCK]",
        value,
    )
    count += private_keys
    for pattern in [r"(?:sk-proj-|sk-|ghp_|github_pat_|AKIA|ASIA)[A-Za-z0-9_.-]{8,}"]:
        output, found = re.subn(pattern, "[REDACTED_SECRET]", output)
        count += found
    assignment = re.compile(
        r"(?im)(api_?key|secret_key|client_secret|access_token|auth_token|password|passwd)(\s*[:=]\s*)([^\s,;]{8,})"
    )

    def replace_assignment(match: re.Match[str]) -> str:
        nonlocal count
        if match.group(3).startswith("[REDACTED"):
            return match.group(0)
        count += 1
        return f"{match.group(1)}{match.group(2)}[REDACTED_SECRET]"

    output = assignment.sub(replace_assignment, output)
    return output, count


def compact_context(root: Path, ranked: list[tuple[int, Symbol]], max_chars: int = 4_800) -> str:
    output: list[str] = []
    used = 0
    for score, symbol in ranked:
        lines = (root / symbol.path).read_text(encoding="utf-8").splitlines()
        start = max(1, symbol.line_start - 2)
        end = min(len(lines), symbol.line_end + 2)
        body: list[str] = []
        for line_number in range(start, end + 1):
            line = lines[line_number - 1]
            stripped = line.strip()
            if not stripped or (stripped.startswith("//") and not stripped.startswith(("///", "//!"))):
                continue
            body.append(f"{line_number:>5} | {' '.join(line.split())}")
        block = f"--- {symbol.path}:{symbol.line_start} {symbol.name} score={score} ---\n" + "\n".join(body) + "\n"
        block, _ = redact(block)
        if used + len(block) > max_chars:
            break
        output.append(block)
        used += len(block)
    return "\n".join(output)


def extract_mcp_schema(root: Path) -> dict:
    source = (root / "src/mcp.rs").read_text(encoding="utf-8")
    marker = 'fn tools_list_json() -> String {'
    start = source.index(marker)
    raw_start = source.index('r#"', start) + 3
    raw_end = source.index('"#', raw_start)
    return json.loads(source[raw_start:raw_end].replace("\n", ""))


def check_manifest(root: Path) -> str:
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    package = manifest["package"]
    assert package["name"] == "codespace-cse"
    assert package["version"] == "2.0.0"
    assert package["edition"] == "2024"
    assert package["license"] == "Apache-2.0"
    assert manifest["bin"][0]["name"] == "cse"
    return f"{package['name']} {package['version']}, Rust {package['rust-version']}+"


def check_structure(root: Path) -> str:
    required = [
        "Cargo.toml", "LICENSE", "NOTICE", "README.md", "SECURITY.md", "CHANGELOG.md",
        "src/main.rs", "src/lib.rs", "src/cli.rs", "src/model.rs", "src/parser.rs",
        "src/storage.rs", "src/indexer.rs", "src/context.rs", "src/mcp.rs",
        "src/application.rs", "src/server.rs", "src/dashboard.rs", "src/workspace.rs",
        "src/settings.rs", "src/events.rs", "src/skills.rs", "src/mcp_manager.rs",
        "tests/library.rs", "tests/cli_smoke.rs", "tests/v2_features.rs",
        "fixtures/sample/src/auth.rs",
        ".github/workflows/ci.yml", ".github/workflows/release.yml",
        ".github/workflows/pages.yml", "site/index.html", "site/styles.css",
        "site/app.js", "scripts/validate_site.py",
    ]
    missing = [path for path in required if not (root / path).exists()]
    assert not missing, f"missing files: {missing}"
    return f"{len(required)} required release assets present"


def check_rust_lexical(root: Path) -> str:
    files = sorted((root / "src").glob("*.rs")) + sorted((root / "tests").glob("*.rs"))
    lines = tokens_count = 0
    for path in files:
        file_lines, file_tokens = assert_balanced_rust(path)
        lines += file_lines
        tokens_count += file_tokens
    return f"{len(files)} Rust files, {lines} lines, balanced delimiters"


def check_modules(root: Path) -> str:
    lib = (root / "src/lib.rs").read_text(encoding="utf-8")
    modules = re.findall(r"^pub mod ([A-Za-z_][A-Za-z0-9_]*);", lib, re.MULTILINE)
    missing = [module for module in modules if not (root / f"src/{module}.rs").exists()]
    assert not missing, f"missing module files: {missing}"
    assert len(modules) >= 12
    return f"{len(modules)} public modules resolve to files"


def check_security_invariants(root: Path) -> str:
    violations: list[str] = []
    for path in sorted((root / "src").glob("*.rs")):
        cleaned = strip_rust_comments_and_strings(path.read_text(encoding="utf-8"))
        for line_number, line in enumerate(cleaned.splitlines(), 1):
            if re.search(r"\bunsafe\s*\{", line):
                violations.append(f"{path.name}:{line_number}: unsafe block")
            if ".unwrap(" in line or ".expect(" in line:
                violations.append(f"{path.name}:{line_number}: panic-prone unwrap/expect")
    assert not violations, "; ".join(violations)
    security = (root / "SECURITY.md").read_text(encoding="utf-8").lower()
    for phrase in ["loopback", "secret", "symlink", "2 mib", "no authentication"]:
        assert phrase in security, f"SECURITY.md missing {phrase!r}"
    return "no unsafe blocks or unwrap/expect in production; security defaults documented"


def check_mcp(root: Path) -> str:
    schema = extract_mcp_schema(root)
    names = [tool["name"] for tool in schema["tools"]]
    assert names == ["cse_search", "cse_context", "cse_impact", "cse_history", "cse_read"]
    for tool in schema["tools"]:
        assert tool["inputSchema"]["type"] == "object"
        assert tool["description"]
    source = (root / "src/mcp.rs").read_text(encoding="utf-8")
    assert '"2025-11-25"' in source and '"2025-06-18"' in source
    return f"valid JSON schemas for {len(names)} tools; version negotiation present"


def check_self_index(root: Path, metrics: dict) -> str:
    files = source_files(root)
    symbols, calls = parse_symbols(root, files)
    edges = build_edges(symbols, calls)
    assert len(files) >= 25, len(files)
    assert len(symbols) >= 80, len(symbols)
    assert len(edges) >= 20, len(edges)
    ranked = rank(symbols, edges, "MCP context impact secret redaction", 15)
    assert ranked, "no ranked context"
    relevant = {symbol.path for _, symbol in ranked}
    expected = {"src/mcp.rs", "src/context.rs", "src/impact.rs", "src/secret.rs"}
    assert len(relevant & expected) >= 3, f"top context lacked core modules: {sorted(relevant)}"
    context = compact_context(root, ranked)
    assert context
    full_chars = sum(path.stat().st_size for path in files)
    reduction = 1.0 - (len(context.encode()) / max(1, full_chars))
    assert reduction > 0.70, reduction
    metrics.update({
        "indexed_files": len(files), "symbols": len(symbols), "call_edges": len(edges),
        "full_source_bytes": full_chars, "context_bytes": len(context.encode()),
        "reference_context_reduction_pct": round(reduction * 100, 2),
        "top_context_paths": sorted(relevant)[:15],
    })
    (root / "artifacts/self-context.txt").write_text(context, encoding="utf-8")
    return f"indexed self: {len(files)} files, {len(symbols)} symbols, {len(edges)} call edges; context bytes reduced {reduction:.1%}"


def check_incremental_and_redaction(metrics: dict) -> str:
    with tempfile.TemporaryDirectory(prefix="codespace-selftest-") as temp:
        root = Path(temp)
        (root / "a.rs").write_text("pub fn alpha() { beta(); }\npub fn beta() {}\n", encoding="utf-8")
        (root / "b.py").write_text("def login(user):\n    return bool(user)\n", encoding="utf-8")
        files = source_files(root)
        first = {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in files}
        second = {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in source_files(root)}
        unchanged = sum(first[name] == second[name] for name in first)
        assert unchanged == 2
        (root / "a.rs").write_text("pub fn alpha() { beta(); }\npub fn beta() { gamma(); }\npub fn gamma() {}\n", encoding="utf-8")
        third = {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in source_files(root)}
        changed = [name for name in first if first[name] != third[name]]
        assert changed == ["a.rs"]
    sample = (
        "OPENAI_API_KEY=sk-proj-1234567890abcdefgh\n"
        "password: correct-horse-battery-staple\n"
        "ghp_1234567890abcdefgh\n"
        "-----BEGIN PRIVATE KEY-----\nprivate-material\n-----END PRIVATE KEY-----"
    )
    cleaned, redactions = redact(sample)
    assert redactions == 4, redactions
    assert "1234567890" not in cleaned and "correct-horse" not in cleaned
    assert "private-material" not in cleaned
    metrics.update({"incremental_unchanged_files": unchanged, "incremental_changed_files": changed, "redaction_count": redactions})
    return "unchanged hashes preserved; one-file modification isolated; 4/4 secrets redacted"


def check_git_diff(metrics: dict) -> str:
    if shutil.which("git") is None:
        return "git unavailable; skipped"
    with tempfile.TemporaryDirectory(prefix="codespace-git-") as temp:
        root = Path(temp)
        subprocess.run(["git", "init", "-q"], cwd=root, check=True)
        subprocess.run(["git", "config", "user.email", "selftest@example.invalid"], cwd=root, check=True)
        subprocess.run(["git", "config", "user.name", "CodeSpace Self-Test"], cwd=root, check=True)
        path = root / "auth.rs"
        path.write_text("pub fn login(user: &str) -> bool { !user.is_empty() }\n", encoding="utf-8")
        subprocess.run(["git", "add", "."], cwd=root, check=True)
        subprocess.run(["git", "commit", "-qm", "baseline"], cwd=root, check=True)
        path.write_text("pub fn login(user: &str) -> bool { user.len() >= 2 }\n", encoding="utf-8")
        subprocess.run(["git", "add", "."], cwd=root, check=True)
        subprocess.run(["git", "commit", "-qm", "change login"], cwd=root, check=True)
        diff = subprocess.run(["git", "diff", "--unified=0", "HEAD~1", "HEAD", "--"], cwd=root, check=True, capture_output=True, text=True).stdout
        assert "+++ b/auth.rs" in diff
        hunks = re.findall(r"@@ -[^ ]+ \+(\d+)(?:,(\d+))? @@", diff)
        assert hunks and int(hunks[0][0]) == 1
        metrics["git_changed_file"] = "auth.rs"
        metrics["git_new_hunks"] = hunks
    return "Git unified diff mapped changed auth.rs line range"


def write_reports(root: Path, suite: Suite, metrics: dict, started: float) -> None:
    report = {
        "release": "2.0.0",
        "generated_unix_ms": int(time.time() * 1000),
        "elapsed_ms": int((time.perf_counter() - started) * 1000),
        "status": "pass" if not suite.failures else "fail",
        "checks": [asdict(check) for check in suite.checks],
        "metrics": metrics,
        "limitations": [
            "This Python harness does not compile or execute the Rust binary.",
            "Rust compilation, rustfmt, Clippy, unit, integration, and MCP process tests must run under Cargo/CI.",
            "Reference context-reduction metrics measure byte volume on this repository, not model accuracy or end-to-end development speed.",
        ],
    }
    artifacts = root / "artifacts"
    artifacts.mkdir(exist_ok=True)
    (artifacts / "self-test-report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    rows = [
        "# CodeSpace 1.0 self-test report", "", f"Status: **{report['status'].upper()}**", "",
        "| Check | Status | Evidence | Duration |", "|---|---:|---|---:|",
    ]
    for check in suite.checks:
        detail = check.detail.replace("|", "\\|").replace("\n", " ")
        rows.append(f"| {check.name} | {check.status} | {detail} | {check.duration_ms} ms |")
    rows += ["", "## Metrics", "", "```json", json.dumps(metrics, ensure_ascii=False, indent=2), "```", "", "## Validation boundary", ""]
    rows.extend(f"- {item}" for item in report["limitations"])
    (artifacts / "SELF_TEST.md").write_text("\n".join(rows) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    root = args.root.resolve()
    started = time.perf_counter()
    suite = Suite()
    metrics: dict = {}

    suite.run("manifest", lambda: check_manifest(root))
    suite.run("release structure", lambda: check_structure(root))
    suite.run("Rust lexical balance", lambda: check_rust_lexical(root))
    suite.run("module resolution", lambda: check_modules(root))
    suite.run("security invariants", lambda: check_security_invariants(root))
    suite.run("MCP schemas", lambda: check_mcp(root))
    suite.run("self indexing and context", lambda: check_self_index(root, metrics))
    suite.run("incremental and redaction", lambda: check_incremental_and_redaction(metrics))
    suite.run("Git diff mapping", lambda: check_git_diff(metrics))

    write_reports(root, suite, metrics, started)
    print(f"\nReport: {root / 'artifacts/self-test-report.json'}")
    return 1 if suite.failures else 0


if __name__ == "__main__":
    sys.exit(main())
