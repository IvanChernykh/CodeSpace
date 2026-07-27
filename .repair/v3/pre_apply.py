#!/usr/bin/env python3
from pathlib import Path

path = Path("src/server.rs")
text = path.read_text(encoding="utf-8")
old = "            workspaces: load_global_registry(),\n            events,\n        }"
new = "            workspaces: load_global_registry(),\n            events,\n            skills: crate::skills::load_skill_registry(),\n            mcp: crate::mcp_manager::load_mcp_manager(),\n        }"
count = text.count(old)
if count != 1:
    raise SystemExit(f"pre-apply ServerState initializer: expected one match, found {count}")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
