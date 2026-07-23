# CodeSpace 1.0 self-test report

Status: **PASS**

| Check | Status | Evidence | Duration |
|---|---:|---|---:|
| manifest | pass | codespace-cse 1.0.0, Rust 1.85+ | 0 ms |
| release structure | pass | 25 required release assets present | 0 ms |
| Rust lexical balance | pass | 18 Rust files, 4842 lines, balanced delimiters | 185 ms |
| module resolution | pass | 14 public modules resolve to files | 0 ms |
| security invariants | pass | no unsafe blocks or unwrap/expect in production; security defaults documented | 177 ms |
| MCP schemas | pass | valid JSON schemas for 5 tools; version negotiation present | 0 ms |
| self indexing and context | pass | indexed self: 47 files, 301 symbols, 378 call edges; context bytes reduced 98.6% | 33 ms |
| incremental and redaction | pass | unchanged hashes preserved; one-file modification isolated; 4/4 secrets redacted | 0 ms |
| Git diff mapping | pass | Git unified diff mapped changed auth.rs line range | 15 ms |

## Metrics

```json
{
  "indexed_files": 47,
  "symbols": 301,
  "call_edges": 378,
  "full_source_bytes": 245988,
  "context_bytes": 3509,
  "reference_context_reduction_pct": 98.57,
  "top_context_paths": [
    "src/cli.rs",
    "src/context.rs",
    "src/impact.rs",
    "src/indexer.rs",
    "src/mcp.rs",
    "src/secret.rs",
    "src/storage.rs",
    "tests/library.rs"
  ],
  "incremental_unchanged_files": 2,
  "incremental_changed_files": [
    "a.rs"
  ],
  "redaction_count": 4,
  "git_changed_file": "auth.rs",
  "git_new_hunks": [
    [
      "1",
      ""
    ]
  ]
}
```

## Validation boundary

- This Python harness does not compile or execute the Rust binary.
- Rust compilation, rustfmt, Clippy, unit, integration, and MCP process tests must run under Cargo/CI.
- Reference context-reduction metrics measure byte volume on this repository, not model accuracy or end-to-end development speed.
