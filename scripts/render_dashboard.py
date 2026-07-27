#!/usr/bin/env python3
"""Render the real localhost dashboard with a headless Chromium browser."""

from __future__ import annotations

import os
import shutil
import struct
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HOST = "127.0.0.1"
PORT = 18081


def chrome_binary() -> str:
    for name in ("google-chrome", "google-chrome-stable", "chromium", "chromium-browser"):
        resolved = shutil.which(name)
        if resolved:
            return resolved
    raise RuntimeError("a Chromium-compatible browser is required for visual dashboard validation")


def wait_for_server(process: subprocess.Popen[str]) -> None:
    if process.stderr is None:
        raise RuntimeError("dashboard stderr is unavailable")
    deadline = time.monotonic() + 20
    lines: list[str] = []
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError("dashboard exited before render:\n" + "\n".join(lines[-20:]))
        line = process.stderr.readline()
        if line:
            stripped = line.rstrip()
            lines.append(stripped)
            if "CodeSpace server listening on" in stripped:
                return
        time.sleep(0.05)
    raise RuntimeError("dashboard did not become ready:\n" + "\n".join(lines[-20:]))


def png_dimensions(path: Path) -> tuple[int, int]:
    data = path.read_bytes()
    if not data.startswith(b"\x89PNG\r\n\x1a\n") or len(data) < 24:
        raise RuntimeError("browser did not produce a valid PNG screenshot")
    return struct.unpack(">II", data[16:24])


def run() -> None:
    repository = Path(__file__).resolve().parents[1]
    binary = repository / "target" / "debug" / ("cse.exe" if os.name == "nt" else "cse")
    if not binary.is_file():
        raise RuntimeError(f"missing debug binary: {binary}")
    artifacts = repository / "artifacts"
    artifacts.mkdir(exist_ok=True)
    screenshot = artifacts / "dashboard-overview.png"
    dom_path = artifacts / "dashboard-overview.html"

    with tempfile.TemporaryDirectory(prefix="codespace-visual-") as directory:
        workspace = Path(directory)
        (workspace / "src").mkdir()
        (workspace / "src" / "main.rs").write_text(
            "mod engine;\nfn main() { println!(\"{}\", engine::status()); }\n",
            encoding="utf-8",
        )
        (workspace / "src" / "engine.rs").write_text(
            "pub fn status() -> &'static str { \"ready\" }\n",
            encoding="utf-8",
        )
        (workspace / "README.md").write_text("# Visual fixture\n", encoding="utf-8")
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
                str(PORT),
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        try:
            wait_for_server(process)
            browser = chrome_binary()
            url = f"http://{HOST}:{PORT}/"
            common = [
                browser,
                "--headless=new",
                "--no-sandbox",
                "--disable-gpu",
                "--hide-scrollbars",
                "--window-size=1440,1000",
                "--force-device-scale-factor=1",
                "--virtual-time-budget=5000",
            ]
            result = subprocess.run(
                [*common, f"--screenshot={screenshot}", url],
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
            )
            if result.returncode != 0:
                raise RuntimeError(f"Chrome screenshot failed: {result.stderr}")
            dom = subprocess.run(
                [*common, "--dump-dom", url],
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
            )
            if dom.returncode != 0:
                raise RuntimeError(f"Chrome DOM render failed: {dom.stderr}")
            dom_path.write_text(dom.stdout, encoding="utf-8")
            if "CodeSpace — IDE Assistant" not in dom.stdout or "Command Center" not in dom.stdout:
                raise RuntimeError("rendered DOM is missing dashboard identity")
            if 'data-state="online"' not in dom.stdout:
                raise RuntimeError("rendered dashboard did not reach the online state")
            width, height = png_dimensions(screenshot)
            if (width, height) != (1440, 1000):
                raise RuntimeError(f"unexpected screenshot dimensions: {width}x{height}")
            print(f"dashboard visual render passed: {screenshot} ({width}x{height})")
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
    except Exception as error:  # visual gate must report complete failure evidence
        print(f"dashboard visual render failed: {error}", file=sys.stderr)
        raise
