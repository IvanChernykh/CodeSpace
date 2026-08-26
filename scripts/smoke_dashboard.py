#!/usr/bin/env python3
"""Runtime smoke test for the embedded CodeSpace localhost dashboard."""

from __future__ import annotations

import http.client
import json
import os
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path
from typing import Any

HOST = "127.0.0.1"
START_PORT = 18081
ACTIVE_PORT = START_PORT


def request(
    method: str,
    path: str,
    *,
    cookie: str = "",
    payload: dict[str, Any] | None = None,
) -> tuple[int, str, dict[str, str]]:
    body = json.dumps(payload).encode("utf-8") if payload is not None else None
    headers = {"Connection": "close"}
    if cookie:
        headers["Cookie"] = f"cse_session={cookie}"
    if body is not None:
        headers["Content-Type"] = "application/json"
        headers["Content-Length"] = str(len(body))
    connection = http.client.HTTPConnection(HOST, ACTIVE_PORT, timeout=5)
    try:
        connection.request(method, path, body=body, headers=headers)
        response = connection.getresponse()
        content = response.read().decode("utf-8", errors="replace")
        response_headers = {key.lower(): value for key, value in response.getheaders()}
        return response.status, content, response_headers
    finally:
        connection.close()


def wait_for_server(process: subprocess.Popen[str]) -> None:
    global ACTIVE_PORT
    deadline = time.monotonic() + 20
    listening = False
    lines: list[str] = []
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(
                "dashboard process exited before startup:\n" + "\n".join(lines[-30:])
            )
        line = process.stderr.readline()
        if line:
            stripped = line.rstrip()
            lines.append(stripped)
            if stripped.startswith("CodeSpace server listening on http://"):
                try:
                    ACTIVE_PORT = int(stripped.rsplit(":", 1)[1])
                except ValueError as error:
                    raise RuntimeError(f"invalid server address: {stripped}") from error
                listening = True
        if listening:
            try:
                status, body, _ = request("GET", "/api/v1/health")
                if status == 200 and '"status":"ok"' in body:
                    return
            except OSError:
                pass
        time.sleep(0.05)
    raise RuntimeError("dashboard did not become ready:\n" + "\n".join(lines[-30:]))


def assert_json(status: int, body: str, expected_status: int = 200) -> dict[str, Any]:
    if status != expected_status:
        raise AssertionError(f"expected HTTP {expected_status}, received {status}: {body}")
    parsed = json.loads(body)
    if not isinstance(parsed, dict):
        raise AssertionError(f"expected JSON object, received: {body}")
    return parsed


def run() -> None:
    repository = Path(__file__).resolve().parents[1]
    binary_override = os.environ.get("CSE_BINARY")
    binary = (
        Path(binary_override)
        if binary_override
        else repository / "target" / "debug" / ("cse.exe" if os.name == "nt" else "cse")
    )
    if not binary.exists():
        raise RuntimeError(f"missing debug binary: {binary}")

    with tempfile.TemporaryDirectory(prefix="codespace-dashboard-smoke-") as directory:
        workspace = Path(directory)
        (workspace / "src").mkdir()
        (workspace / "src" / "lib.rs").write_text(
            "pub fn smoke_value() -> usize { 42 }\n",
            encoding="utf-8",
        )
        (workspace / "CODESPACE.md").write_text(
            """<!-- cse:cell
id: smoke.local-contract
status: accepted
priority: critical
tags: smoke, local
paths: src/lib.rs
symbols: smoke_value
-->
The smoke value is local repository evidence.
<!-- /cse:cell -->
""",
            encoding="utf-8",
        )
        subprocess.run(
            [str(binary), "init", "--path", str(workspace), "--force"],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

        process = subprocess.Popen(
            [
                str(binary),
                "serve",
                "--dashboard",
                "--path",
                str(workspace),
                "--port",
                str(START_PORT),
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        if process.stderr is None:
            raise RuntimeError("failed to capture dashboard stderr")
        try:
            wait_for_server(process)

            status, html, headers = request("GET", "/")
            if status != 200 or "CodeSpace — IDE Assistant" not in html:
                raise AssertionError(f"dashboard HTML failed: HTTP {status}")
            if html.count("<script") != 1:
                raise AssertionError("dashboard must expose exactly one frontend runtime")
            if "content-security-policy" not in headers:
                raise AssertionError("dashboard response is missing Content-Security-Policy")
            cookie = headers.get("set-cookie", "")
            if "HttpOnly" not in cookie or "SameSite=Strict" not in cookie:
                raise AssertionError("dashboard session cookie is missing strict browser flags")
            session_cookie = cookie.partition("cse_session=")[2].partition(";")[0]
            if len(session_cookie) < 32:
                raise AssertionError("dashboard session cookie is unexpectedly short")

            for asset in ("/assets/main.js", "/assets/dashboard.css"):
                asset_status, asset_body, _ = request("GET", asset)
                if asset_status != 200 or len(asset_body) < 500:
                    raise AssertionError(f"embedded asset failed: {asset}")

            unauthorized_status, _, _ = request("GET", "/api/v1/graph")
            if unauthorized_status != 401:
                raise AssertionError(
                    f"protected graph endpoint returned {unauthorized_status} without token"
                )

            graph = assert_json(
                *request("GET", "/api/v1/graph", cookie=session_cookie)[:2]
            )
            if not graph.get("files") or not graph.get("symbols"):
                raise AssertionError("graph endpoint returned an empty initialized workspace")
            stats = assert_json(
                *request("GET", "/api/v1/stats", cookie=session_cookie)[:2]
            )
            if int(stats.get("files", 0)) < 1:
                raise AssertionError("stats endpoint did not report indexed files")
            context = assert_json(
                *request(
                    "GET",
                    "/api/v1/context?q=local+smoke&max_tokens=500&max_items=4",
                    cookie=session_cookie,
                )[:2]
            )
            if not str(context.get("pack_id", "")).startswith("ctx-"):
                raise AssertionError("context endpoint did not return a stable pack id")
            if not context.get("cells") or not context.get("metrics"):
                raise AssertionError("context endpoint did not return cells and metrics")
            if int(context.get("estimated_tokens", 501)) > 500:
                raise AssertionError("context endpoint exceeded its hard token budget")

            long_title = "HTTP body " + ("x" * 32768)
            task = assert_json(
                *request(
                    "POST",
                    "/api/v1/tasks",
                    cookie=session_cookie,
                    payload={"title": long_title, "priority": "high", "tags": "smoke,http"},
                )[:2]
            )
            if not task.get("id"):
                raise AssertionError("task creation did not return an id")
            tasks = assert_json(
                *request("GET", "/api/v1/tasks", cookie=session_cookie)[:2]
            )
            records = tasks.get("tasks")
            if not isinstance(records, list) or not any(
                isinstance(item, dict) and item.get("title") == long_title for item in records
            ):
                raise AssertionError("server did not read and persist the complete POST body")

            for endpoint, key in (
                ("/api/v1/skills", "skills"),
                ("/api/v1/mcp", "servers"),
                ("/api/v1/settings", "effective"),
            ):
                payload = assert_json(
                    *request("GET", endpoint, cookie=session_cookie)[:2]
                )
                if key not in payload:
                    raise AssertionError(f"{endpoint} is missing expected key {key}")

            print("dashboard runtime smoke test passed")
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)


if __name__ == "__main__":
    try:
        run()
    except Exception as error:  # noqa: BLE001 - test runner must report all failures
        print(f"dashboard runtime smoke test failed: {error}", file=sys.stderr)
        raise
