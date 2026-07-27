#!/usr/bin/env python3
from pathlib import Path

path = Path("src/cli.rs")
text = path.read_text(encoding="utf-8")

workspace_old = '''            if args.value("format").is_some_and(|f| f == "json") {
                println!("{}", registry.to_json());
            } else {
                if registry.list().is_empty() {
                    println!("No workspaces registered. Use `cse workspace register <path>` to add one.");
                } else {
                    for ws in registry.list() {
                        let active = registry.active_id.as_deref() == Some(ws.id.as_str());
                        println!(
                            "{}  {}  {}{}",
                            if active { "*" } else { " " },
                            ws.name,
                            ws.path,
                            if active { " (active)" } else { "" }
                        );
                    }
                }
            }
'''
workspace_new = '''            if args.value("format").is_some_and(|f| f == "json") {
                println!("{}", registry.to_json());
            } else if registry.list().is_empty() {
                println!("No workspaces registered. Use `cse workspace register <path>` to add one.");
            } else {
                for ws in registry.list() {
                    let active = registry.active_id.as_deref() == Some(ws.id.as_str());
                    println!(
                        "{}  {}  {}{}",
                        if active { "*" } else { " " },
                        ws.name,
                        ws.path,
                        if active { " (active)" } else { "" }
                    );
                }
            }
'''

skills_old = '''            if args.value("format").is_some_and(|f| f == "json") {
                println!("{}", registry.to_json());
            } else {
                if registry.list().is_empty() {
                    println!("No skills installed.");
                } else {
                    for skill in registry.list() {
                        println!(
                            "{}  {}  v{}  {}  {}",
                            if skill.enabled { "+" } else { "-" },
                            skill.manifest.name,
                            skill.manifest.version,
                            skill.manifest.description,
                            skill.source
                        );
                    }
                }
            }
'''
skills_new = '''            if args.value("format").is_some_and(|f| f == "json") {
                println!("{}", registry.to_json());
            } else if registry.list().is_empty() {
                println!("No skills installed.");
            } else {
                for skill in registry.list() {
                    println!(
                        "{}  {}  v{}  {}  {}",
                        if skill.enabled { "+" } else { "-" },
                        skill.manifest.name,
                        skill.manifest.version,
                        skill.manifest.description,
                        skill.source
                    );
                }
            }
'''

for label, old, new in [
    ("workspace list", workspace_old, workspace_new),
    ("skills list", skills_old, skills_new),
]:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    text = text.replace(old, new, 1)

path.write_text(text, encoding="utf-8")
