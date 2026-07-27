#!/usr/bin/env python3
"""Make successful bootstrap health immediately confirm the dashboard connection state."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PATH = ROOT / "dashboard/src/main.ts"


def replace_once(content: str, old: str, new: str) -> str:
    count = content.count(old)
    if count != 1:
        raise SystemExit(f"expected one TypeScript match, found {count}")
    return content.replace(old, new, 1)


content = PATH.read_text(encoding="utf-8")
content = replace_once(
    content,
    '''  constructor(private readonly api: ApiClient, private readonly onEvent: (event: JsonObject) => void) {}

  start(): void {''',
    '''  constructor(private readonly api: ApiClient, private readonly onEvent: (event: JsonObject) => void) {}

  confirmOnline(): void {
    this.lastEventAt = Date.now();
    this.setState("online", "Local runtime healthy");
  }

  start(): void {''',
)
content = replace_once(
    content,
    '''    if (health) $("#runtimeVersion").textContent = `v${health.version}`;''',
    '''    if (health) {
      $("#runtimeVersion").textContent = `v${health.version}`;
      this.connection.confirmOnline();
    }''',
)
PATH.write_text(content, encoding="utf-8")
print("bootstrap health now confirms online state")
