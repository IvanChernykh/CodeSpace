#!/usr/bin/env python3
"""End-to-end smoke test for the embedded CodeSpace dashboard.

Uses only the Python standard library. The test starts the freshly built binary
against an isolated temporary workspace and verifies the browser bootstrap,
public assets, authenticated APIs, task persistence, and clean shutdown.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import signal
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

TOKEN_RE = re.compile(r"(?:session token|token)\s*[:=]\s*([A-Za-z0-9_-]{16,})", re.I)
URL_RE = re.compile(r"http://127\.0\.0\.1:(\d+)")


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def request(url: str, token: str | None = None, method: str = "GET", body: Any = None) -> tuple[int, bytes, dict[str, str]]:
    headers = {"Accept": "application/json, text/html;q=0.9, */*;q=0.8"}
    data = None
    if token:
        headers["Authorization"] = f"Bearer {token}"
    if body is not None:
        data = json.dumps(body).encode("utf-8")
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=8) as response:
            return response.status, response.read(), {k.lower(): v for k, v in response.headers.items()}
    except urllib.error.HTTPError as error:
        return error.code, error.read(), {k.lower(): v for k, v in error.headers.items()}


def wait_for_server(base_url: str, process: subprocess.Popen[str], timeout: float = 20.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"server exited early with code {process.returncode}")
        try:
            status, _, _ = request(f"{base_url}/api/v1/health")
            if status in (200, 401):
                return
        except OSError:
            pass
        time.sleep(0.2)
    raise TimeoutError(f"server did not become ready at {base_url}")


def assert_status(actual: int, expected: int, label: str) -> None:
    if actual != expected:
        raise AssertionError(f"{label}: expected HTTP {expected}, got {actual}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, help="Path to the freshly built cse binary")
    args = parser.parse_args()

    binary = str(Path(args.binary).resolve())
    port = free_port()
    base_url = f"http://127.0.0.1:{port}"

    with tempfile.TemporaryDirectory(prefix="codespace-dashboard-") as tmp:
        workspace = Path(tmp)
        (workspace / "src").mkdir()
        (workspace / "src" / "lib.rs").write_text(
            "pub fn alpha() -> usize { beta() }\npub fn beta() -> usize { 42 }\n",
            encoding="utf-8",
        )
        subprocess.run([binary, "init", "--path", str(workspace)], check=True, capture_output=True, text=True)

        command = [
            binary,
            "serve",
            "--rest",
            "--dashboard",
            "--port",
            str(port),
            "--path",
            str(workspace),
        ]
        process = subprocess.Popen(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
            env={**os.environ, "NO_COLOR": "1"},
        )
        captured: list[str] = []
        token = ""
        try:
            deadline = time.monotonic() + 20
            while time.monotonic() < deadline and not token:
                if process.poll() is not None:
                    raise RuntimeError(f"server exited with code {process.returncode}: {''.join(captured)}")
                assert process.stdout is not None
                line = process.stdout.readline()
                if line:
                    captured.append(line)
                    match = TOKEN_RE.search(line)
                    if match:
                        token = match.group(1)
                    url_match = URL_RE.search(line)
                    if url_match and int(url_match.group(1)) != port:
                        raise AssertionError(f"server silently changed requested free port: {line.strip()}")
                else:
                    time.sleep(0.05)

            wait_for_server(base_url, process)
            if not token:
                raise AssertionError("server did not print a session token")

            status, html, headers = request(f"{base_url}/")
            assert_status(status, 200, "dashboard HTML")
            html_text = html.decode("utf-8", errors="replace")
            for marker in ("CodeSpace", "__CSE_TOKEN__", "/assets/main.js", "/assets/dashboard.css"):
                if marker not in html_text:
                    raise AssertionError(f"dashboard HTML missing bootstrap marker: {marker}")
            if "text/html" not in headers.get("content-type", ""):
                raise AssertionError("dashboard root has an invalid Content-Type")

            for asset, expected_type in (("main.js", "javascript"), ("dashboard.css", "css")):
                status, content, asset_headers = request(f"{base_url}/assets/{asset}")
                assert_status(status, 200, f"public asset {asset}")
                if len(content) < 100:
                    raise AssertionError(f"asset {asset} is unexpectedly small")
                if expected_type not in asset_headers.get("content-type", "").lower():
                    raise AssertionError(f"asset {asset} has invalid Content-Type")

            status, _, _ = request(f"{base_url}/api/v1/graph")
            assert_status(status, 401, "protected graph without token")

            for endpoint in ("health", "graph", "workspaces", "tasks", "github/status"):
                status, payload, _ = request(f"{base_url}/api/v1/{endpoint}", token=token)
                assert_status(status, 200, f"authenticated {endpoint}")
                if not payload:
                    raise AssertionError(f"endpoint {endpoint} returned an empty body")

            task = {
                "title": "Dashboard smoke task",
                "description": "Created by scripts/dashboard_smoke.py",
                "priority": "medium",
                "tags": ["smoke", "dashboard"],
            }
            status, created, _ = request(f"{base_url}/api/v1/tasks", token=token, method="POST", body=task)
            if status not in (200, 201):
                raise AssertionError(f"task creation failed: HTTP {status}: {created.decode(errors='replace')}")
            status, listed, _ = request(f"{base_url}/api/v1/tasks", token=token)
            assert_status(status, 200, "task listing")
            if "Dashboard smoke task" not in listed.decode("utf-8", errors="replace"):
                raise AssertionError("created task is not visible in task listing")

            print("dashboard smoke: PASS")
            return 0
        finally:
            if process.poll() is None:
                if os.name == "nt":
                    process.terminate()
                else:
                    process.send_signal(signal.SIGTERM)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
            if process.returncode not in (0, -signal.SIGTERM, 143, 1):
                print("server output:\n" + "".join(captured))


if __name__ == "__main__":
    raise SystemExit(main())
