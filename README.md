<div align="center">

# CodeSpace 2.0

**Local-first IDE assistant with semantic code graph, dashboard, MCP, and skills.**

[![CI](https://github.com/IvanChernykh/CodeSpace/actions/workflows/ci.yml/badge.svg)](https://github.com/IvanChernykh/CodeSpace/actions/workflows/ci.yml)
[![Pages](https://github.com/IvanChernykh/CodeSpace/actions/workflows/pages.yml/badge.svg)](https://ivanchernykh.github.io/CodeSpace/)
[![License](https://img.shields.io/badge/license-Apache--2.0-65f6d4.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.97.1-orange.svg)](https://www.rust-lang.org/)
[![Version](https://img.shields.io/badge/version-2.0.0-blue.svg)](CHANGELOG.md)

[Features](#features) &bull; [Install](#install) &bull; [Quick Start](#quick-start) &bull; [Dashboard](#dashboard) &bull; [MCP](#mcp) &bull; [Security](#security)

</div>

---

## Overview

`cse` is a **zero-dependency, local-first** IDE assistant written in Rust. It indexes your repository, extracts symbols and relationships, ranks task-relevant code, redacts secrets, analyzes Git impact, and exposes everything through **CLI, MCP, REST, an embedded dashboard, and a Rust library** — all without sending a single byte to the cloud.

CodeSpace 2.0 introduces:
- **Action Registry** — unified dispatch for all interfaces
- **Dashboard** — embedded web UI with graph canvas and live search
- **Workspace Manager** — multi-repository registration and switching
- **Skills Platform** — manifest-based skills with permissions
- **Local Daemon** — session-secured HTTP server with SSE events
- **Settings System** — layered global/workspace/session configuration

> **No source code leaves your machine. No model or embedding provider required. No database runtime needed.**

## Features

| Area | What it does |
|---|---|
| **Semantic Graph** | Files, symbols, edges (calls, imports, extends, test-covers, configures), precision tiers, evidence |
| **12 Languages** | Rust, Python, JS/TS, Go, Java/Kotlin, C/C++, C#, Swift, PHP, Ruby, shell + structural fallback |
| **Action Registry** | 12 typed actions with aliases, categories, unified dispatch across CLI/MCP/REST/dashboard |
| **Dashboard** | Embedded web UI: graph canvas, symbol inspector, live search, workspace selector |
| **Workspace Manager** | Register multiple repos, switch between them, per-workspace settings |
| **Skills Platform** | 6 built-in skills (code-review, test-cov, dep-audit, doc-gen, refactor-trace), permissions system |
| **Settings** | Global → workspace → session priority chain with JSON persistence |
| **Event Bus** | 13 event types with SSE streaming for realtime UI updates |
| **Local Daemon** | Localhost-only HTTP, session token auth, dynamic port selection, SSE events |
| **Context Engine** | Lexical + graph ranking, token budgeting, comment compaction, secret redaction |
| **Impact Analysis** | Git diff mapping, reverse-edge traversal, depth-limited blast radius, risk score |
| **Decision Memory** | Persistent records by file, symbol, session, agent, rationale, tags |
| **Export** | JSON, Graphviz DOT, standalone interactive HTML |
| **Operations** | Watch mode, lock recovery, doctor, stats, benchmark, shell, workspace, skills, settings |

## Website

Project site: **https://ivanchernykh.github.io/CodeSpace/**

The site is a dependency-free static build under [`site/`](site/) and deploys through the official GitHub Pages Actions workflow.

## Install

### Prerequisites

- **Rust 1.85+** (repository pins 1.97.1)

### From source

```bash
git clone https://github.com/IvanChernykh/CodeSpace.git
cd CodeSpace
cargo install --path . --locked
```

### Verify

```bash
cse --version
# cse 2.0.0
```

## Quick Start

```bash
# 1. Index your project
cd my-project
cse init

# 2. Search for symbols
cse find "authenticate" --type function

# 3. Build context for an AI agent
cse context --query "login returns 500 error" --max-tokens 1200

# 4. Analyze Git impact
cse impact --from main --to HEAD

# 5. Store an engineering decision
cse remember \
  --file src/auth.rs \
  --symbol "verify_token" \
  --summary "Switched to constant-time comparison" \
  --rationale "Prevents timing attacks on token validation"

# 6. Retrieve it later
cse history src/auth.rs
```

Watch for changes:

```bash
cse update --watch
```

## Dashboard

Launch the embedded web UI:

```bash
cse serve --dashboard --port 8080
```

Then open `http://localhost:8080` in your browser. The dashboard provides:
- **Graph canvas** — interactive symbol relationship visualization
- **Symbol inspector** — click any node to see details
- **Live search** — real-time symbol lookup
- **Workspace selector** — switch between registered projects
- **Event stream** — SSE-powered live updates

The server binds to **localhost only** with **session token authentication**. If the requested port is busy, it automatically finds a free one.

## Workspace Manager

```bash
cse workspace register ./my-project --name "my-project"
cse workspace list
cse workspace select <id>
cse workspace remove <id>
```

## Skills

```bash
cse skills list
cse skills enable code-review
cse skills disable doc-gen
cse skills uninstall refactor-trace
```

Built-in: `code-review`, `test-cov`, `dep-audit`, `doc-gen`, `refactor-trace`

## Settings

```bash
cse settings list
cse settings set theme dark --scope global
cse settings set language ru --scope workspace
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

## Security

Key security defaults:

- **Local-only** — all processing happens on your machine
- **No cloud** — source files never sent to any service by this binary
- **Secret redaction** — credentials redacted from context and MCP file reads
- **Path traversal protection** — `cse read` blocks `..` and verifies canonical paths stay within project root
- **Session tokens** — dashboard server uses constant-time token comparison
- **Localhost binding** — REST and dashboard bind to `127.0.0.1` only
- **Remote opt-in** — remote REST requires explicit `--allow-remote` flag
- **Symlink protection** — symlinks are not followed during indexing
- **Size limits** — files over 1 MiB skipped, reads capped at 2 MiB
- **Crash-safe writes** — lock, fsync, and atomic temp-file swap

Secret detection is defense-in-depth, not a DLP guarantee. Do not index production credential stores or `.env` files.

## Validation

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D clippy::correctness
cargo test --all-targets
python3 scripts/self_test.py
```

CI runs the same checks and then executes `cse` against its own source tree.

## Project Structure

```text
src/
  cli.rs            command surface and dispatch
  application.rs    Action Registry — unified dispatch core
  parser.rs         language-aware symbol/call/import extraction
  model.rs          graph and report types
  storage.rs        deterministic crash-aware persistence
  indexer.rs        traversal, ignore rules, incremental updates, watch
  search.rs         lexical and graph ranking
  context.rs        compaction and token budgeting
  secret.rs         credential redaction
  impact.rs         Git diff and blast radius
  memory.rs         engineering decision history
  mcp.rs            MCP stdio server
  rest.rs           local REST API
  server.rs         local daemon with session security and dynamic port
  dashboard.rs      embedded web UI
  workspace.rs      multi-repository workspace manager
  skills.rs         skills platform with permissions
  settings.rs       layered settings (global/workspace/session)
  events.rs         event bus with SSE streaming
  mcp_manager.rs    external MCP server lifecycle
  export.rs         JSON, DOT, HTML
  util.rs           shared utilities
```

## License

Apache-2.0. Clean-room implementation — no source copied from referenced projects.

<div align="center">

---

Made with Rust. Local-first, cloud-free, privacy-respecting.

[Report Bug](https://github.com/IvanChernykh/CodeSpace/issues) &bull; [Request Feature](https://github.com/IvanChernykh/CodeSpace/issues) &bull; [Changelog](CHANGELOG.md)

</div>
