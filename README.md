# CodeSpace 2.0.0

[![CI](https://github.com/IvanChernykh/CodeSpace/actions/workflows/ci.yml/badge.svg)](https://github.com/IvanChernykh/CodeSpace/actions/workflows/ci.yml) [![Pages](https://github.com/IvanChernykh/CodeSpace/actions/workflows/pages.yml/badge.svg)](https://ivanchernykh.github.io/CodeSpace/) [![License](https://img.shields.io/badge/license-Apache--2.0-65f6d4.svg)](LICENSE)

**Local-first advanced IDE assistant with semantic graph, dashboard, MCP, and skills.**

`cse` indexes a repository, extracts symbols and relationships, ranks task-relevant code, redacts likely secrets, analyzes Git change impact, preserves engineering decisions, and exposes the result through CLI, MCP, REST, a local dashboard, and a Rust library. CodeSpace 2.0 adds an Action Registry for unified dispatch, a workspace manager, a skills platform, an embedded web UI, and a local daemon with session security.

> **Naming note:** the product name overlaps semantically with GitHub Codespaces. The collision-prone `cs` command is intentionally not used; the installed binary and MCP prefix are `cse`. Formal trademark and package-name clearance is still required before commercial promotion.

## Release status

Version 2.0.0 is a complete, dependency-free reference implementation of the full IDE assistant platform. It is intentionally conservative:

- no source code leaves the machine;
- no model or embedding provider is required;
- no native database or runtime dependency is required;
- the on-disk index is deterministic, human-inspectable, and replaced through a synchronized temporary-file swap;
- parsing uses language-aware structural heuristics rather than claiming full compiler-level precision;
- measured results are reported by `cse benchmark`; token/cost/speed multipliers are not hard-coded product claims.

The next precision tier is documented in [`docs/ROADMAP.md`](docs/ROADMAP.md): Tree-sitter adapters, LSP resolution, SQLite/WAL, and typed MCP SDK integration.

## Capabilities

| Area | 2.0 implementation |
|---|---|
| Repository index | Incremental hashing, ignored directories, size limits, symlink protection, index revisions |
| Languages | Rust, Python, JS/TS, Go, Java/Kotlin, C/C++, C#, Swift, PHP, Ruby, shell, and structural fallback formats |
| Graph | Files, symbols, containment, imports, calls, extends, test-covers, configures, depends-on, precision tiers, evidence |
| Action Registry | Unified dispatch for CLI, MCP, REST, and dashboard |
| Dashboard | Embedded web UI with graph canvas, inspector, search, workspace selector |
| Workspace Manager | Multi-repository registration, selection, per-workspace settings |
| Skills Platform | Manifest-based skills with permissions, built-in catalog, enable/disable |
| Settings | Global, workspace, and session settings with priority chain |
| Event Bus | Realtime synchronization with state versions |
| Local Daemon | Single-instance, localhost-only, session token, SSE events |
| MCP Management | CodeSpace MCP server + external server lifecycle |
| Context | Lexical + graph ranking, bounded source excerpts, comment/whitespace compaction, token budget |
| Impact | Git unified-diff mapping, reverse-edge traversal, depth limit, risk score |
| Memory | Persistent decision records by file, symbol, session, agent, rationale, tags |
| Interfaces | CLI, MCP stdio, loopback REST, local dashboard, Rust library |
| Export | JSON, Graphviz DOT, standalone interactive HTML |
| Operations | Watch mode, lock recovery, doctor, stats, benchmark, shell, workspace, skills, settings |

## Website

Project site: **https://ivanchernykh.github.io/CodeSpace/**

The site is a dependency-free static build under [`site/`](site/) and deploys through the official GitHub Pages Actions workflow.

## Install

Requires Rust **1.85+**; the repository pins **1.97.1**.

```bash
cargo install --path .
```

The installed command is:

```bash
cse --version
```

Release build:

```bash
cargo build --release
./target/release/cse --version
```

## Quick start

```bash
cd my-project
cse init
cse context --query "authentication returns 500" --max-tokens 1200
cse find login --type function
cse impact --from main --to HEAD
```

Watch for changes:

```bash
cse update --watch
```

Store a decision:

```bash
cse remember \
  --file src/core/engine.rs \
  --symbol Engine::execute \
  --summary "Keep execution deterministic" \
  --rationale "Reproducible impact reports and cache keys" \
  --agent codex \
  --tags architecture,determinism
```

Read it later:

```bash
cse history src/core/engine.rs
```

## MCP

Add one server entry to the MCP client configuration:

```json
{
  "mcpServers": {
    "codespace": {
      "command": "cse",
      "args": ["serve", "--mcp"]
    }
  }
}
```

Exported tools:

- `cse_search`
- `cse_context`
- `cse_impact`
- `cse_history`
- `cse_read`

`cse_read` confines paths to the project root, blocks internal Git/index metadata, enforces a 2 MiB limit, and redacts likely credentials.

## REST

```bash
cse serve --rest --port 8080
```

Endpoints:

```text
GET /v1/health
GET /v1/search?q=login&limit=20
GET /v1/context?q=authentication&max_tokens=1200
```

The server binds to `127.0.0.1` by default. Remote binding requires `--allow-remote` and is **not authenticated**; place it behind a trusted gateway before any network exposure.

## Library

```rust
use codespace::{build_context, load_index, ContextOptions};
use std::path::Path;

let root = Path::new(".");
let graph = load_index(root)?;
let context = build_context(
    root,
    &graph,
    "authentication failure",
    &ContextOptions::default(),
)?;
# Ok::<(), codespace::model::Error>(())
```

## Measurement

Run the engine against its own repository:

```bash
cse init --force
cse benchmark --query "MCP context impact secret redaction" --iterations 100
```

For comparative token tests, use the protocol in [`docs/BENCHMARKS.md`](docs/BENCHMARKS.md). A valid claim requires a fixed repository commit, identical tasks, identical model/settings, repeated trials, and published raw results.


`cse impact` maps deleted or unsupported-language files as file-level pseudo-nodes. It cannot recover symbol-level declarations from a file that no longer exists in the working tree unless the relevant source is present in the indexed snapshot.

## Security model

Read [`SECURITY.md`](SECURITY.md). Key defaults:

- local-only processing;
- source files never sent to a service by this binary;
- secrets are redacted from context and MCP file reads;
- generated/vendor/hidden index directories are skipped;
- symlinks are not followed;
- write operations use a lock, fsync, and recoverable replacement;
- remote REST exposure is opt-in and explicitly warned.

Secret detection is defense-in-depth, not a DLP guarantee. Do not index production credential stores or `.env` files.

## Validation

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D clippy::correctness
cargo test --all-targets
python3 scripts/self_test.py
```

CI runs the same checks and then executes `cse` against its own source tree.

## Project structure

```text
src/
  cli.rs        command surface
  parser.rs     language-aware symbol/call/import extraction
  model.rs      graph and report types
  storage.rs    deterministic crash-aware persistence
  indexer.rs    traversal, ignore rules, incremental updates, watch
  search.rs     lexical and graph ranking
  context.rs    compaction and token budgeting
  secret.rs     credential redaction
  impact.rs     Git diff and blast radius
  memory.rs     engineering decision history
  mcp.rs        MCP stdio server
  rest.rs       local REST API
  export.rs     JSON, DOT, HTML
```

## License and provenance

Apache-2.0. This is a clean-room implementation. No source was copied from the referenced inspiration projects. Before integrating third-party code, record its exact commit, license, NOTICE obligations, and compatibility decision in a software bill of materials.

## Non-goals of 1.0

This release does not claim compiler-equivalent cross-language resolution, embeddings, automatic LSP process management, multi-repository federation, authenticated remote service operation, or independently verified 70–99% token savings. Those require the acceptance evidence defined in the roadmap.
