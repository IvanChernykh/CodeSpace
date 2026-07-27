#!/usr/bin/env python3
from pathlib import Path
import re

root = Path(__file__).resolve().parents[1]
rust = (root / "src/dashboard.rs").read_text(encoding="utf-8")
ts = (root / "dashboard/src/main.ts").read_text(encoding="utf-8")
css = (root / "dashboard/dist/dashboard.css").read_text(encoding="utf-8")
js = (root / "dashboard/dist/main.js").read_text(encoding="utf-8")

match = re.search(r'r##"(<!doctype html>.*?)"##', rust, re.S)
if not match:
    raise SystemExit("dashboard HTML template not found")
html = match.group(1)
html_ids = set(re.findall(r'id="([A-Za-z0-9_-]+)"', html))
static_selectors = set(re.findall(r'\$\("#([A-Za-z0-9_-]+)"', ts))
dynamic_ids = {"readSelectedFileBtn", "selectedFileSource"}
missing = sorted(static_selectors - html_ids - dynamic_ids)
if missing:
    raise SystemExit(f"dashboard TypeScript references missing static IDs: {missing}")

required_views = {
    "overview", "graph", "context", "impact", "assistant", "tasks",
    "history", "workspaces", "skills", "mcp", "settings", "github",
}
views = set(re.findall(r'data-view="([a-z-]+)"', html))
if required_views - views:
    raise SystemExit(f"dashboard is missing views: {sorted(required_views - views)}")

if html.count("<script") != 1:
    raise SystemExit("dashboard must load exactly one JavaScript runtime")
if "fetch('/api/v1" in html or 'fetch("/api/v1' in html:
    raise SystemExit("inline feature API clients are forbidden")
if 'meta name="cse-session"' not in html:
    raise SystemExit("session token meta bootstrap is missing")
if len(js) < 10_000 or len(css) < 10_000:
    raise SystemExit("compiled dashboard assets are unexpectedly small")
if "FileGraphView" not in ts or "resolve_active_root" in ts:
    raise SystemExit("file graph frontend contract is invalid")

print(f"dashboard contract ok: {len(views)} views, {len(html_ids)} static IDs")
