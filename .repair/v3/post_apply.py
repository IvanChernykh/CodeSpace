#!/usr/bin/env python3
from pathlib import Path

path = Path("src/server.rs")
text = path.read_text(encoding="utf-8")
old = "            let state_guard = state.lock().unwrap_or_else(|error| error.into_inner());\n            state_guard.events.publish(&Event::new(EventType::SettingsChanged, \"\", 0).with_data(\"key\", &key));"
new = "            let mut state_guard = state.lock().unwrap_or_else(|error| error.into_inner());\n            state_guard.events.publish(&Event::new(EventType::SettingsChanged, \"\", 0).with_data(\"key\", &key));"
count = text.count(old)
if count != 1:
    raise SystemExit(f"post-apply settings event guard: expected one match, found {count}")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
