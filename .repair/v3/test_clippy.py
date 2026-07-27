#!/usr/bin/env python3
from pathlib import Path

path = Path("tests/v2_features.rs")
text = path.read_text(encoding="utf-8")
replacements = [
    (
        'registry.active().expect("active").name',
        'registry.active().unwrap_or_else(|| panic!("active workspace missing")).name',
        1,
        "active workspace name",
    ),
    (
        'registry.active().expect("active").id',
        'registry.active().unwrap_or_else(|| panic!("active workspace missing")).id',
        2,
        "active workspace ids",
    ),
    (
        'skills.first().expect("no skills").id.clone()',
        'skills.first().unwrap_or_else(|| panic!("no skills registered")).id.clone()',
        1,
        "first registered skill",
    ),
]
for old, new, expected, label in replacements:
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{label}: expected {expected} matches, found {count}")
    text = text.replace(old, new)
path.write_text(text, encoding="utf-8")
