<div align="center">

# CodeSpace 2.0

**A local-first repository brain that compiles trustworthy, token-efficient context for humans and tools.**

[![CI](https://github.com/IvanChernykh/CodeSpace/actions/workflows/ci.yml/badge.svg)](https://github.com/IvanChernykh/CodeSpace/actions/workflows/ci.yml)
[![Pages](https://github.com/IvanChernykh/CodeSpace/actions/workflows/pages.yml/badge.svg)](https://ivanchernykh.github.io/CodeSpace/)
[![License](https://img.shields.io/badge/license-Apache--2.0-65f6d4.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.97.1-orange.svg)](https://www.rust-lang.org/)
[![Version](https://img.shields.io/badge/version-2.0.0-blue.svg)](CHANGELOG.md)

[Features](#features) &bull; [Install](#install) &bull; [Quick Start](#quick-start) &bull; [Dashboard](#dashboard) &bull; [MCP](#mcp) &bull; [Security](#security)

</div>

---

## Overview

`cse` is a **small-runtime, local-first** repository intelligence engine written in Rust. It combines reviewable Markdown Context Cells with a semantic code graph, compiles deterministic evidence packs, redacts secrets, analyzes Git impact, and exposes the same model through **CLI, MCP, REST, a desktop mode, an embedded dashboard, and a Rust library** — without sending source code to a cloud service.

Git remains the compatible history and transport layer. CodeSpace adds what Git does not model: intent, constraints, decisions, symbol relationships, provenance, token budgets, and pre-change blast radius.

CodeSpace 2.0 introduces:
- **Action Registry** — unified dispatch for all interfaces
- **Dashboard** — embedded web UI with graph canvas and live search
- **Workspace Manager** — multi-repository registration and switching
- **Skills Platform** — manifest-based skills with permissions
- **Local Daemon** — session-secured HTTP server with SSE events
- **Settings System** — layered global/workspace/session configuration
- **Context Cells** — versioned Markdown knowledge with stable IDs, status, priority, tags, code evidence, and hashes
- **Context Packs** — deterministic, token-bounded context with an explicit trust boundary and savings telemetry
- **Desktop Mode** — secure loopback app-window launched with one command

> **No source code leaves your machine. No model or embedding provider required. No database runtime needed.**

## Features

| Area | What it does |
|---|---|
| **Semantic Graph** | Files, symbols, edges (calls, imports, extends, test-covers, configures), precision tiers, evidence |
| **12 Languages** | Rust, Python, JS/TS, Go, Java/Kotlin, C/C++, C#, Swift, PHP, Ruby, shell + structural fallback |
| **Action Registry** | 12 typed actions with aliases, categories, unified dispatch across CLI/MCP/REST/dashboard |
| **Context Engine** | Markdown Context Cells + graph ranking, deterministic pack IDs, provenance, token budgeting, savings metrics, secret redaction |
| **Context Studio** | Inspect the exact knowledge/code split, budget, warnings, evidence paths, hashes, and estimated savings |
| **Desktop Mode** | `cse desktop` opens the loopback UI as a standalone app-window with no session token in URLs or JavaScript |
| **Dashboard** | Embedded desktop-grade UI: Context Studio, graph canvas, symbol inspector, search, workspace selector |
| **Workspace Manager** | Register multiple repos, switch between them, per-workspace settings |
| **Skills Platform** | 6 built-in skills (code-review, test-cov, dep-audit, doc-gen, refactor-trace), permissions system |
| **Settings** | Global → workspace → session priority chain with JSON persistence |
| **Event Bus** | 13 event types with SSE streaming for realtime UI updates |
| **Local Daemon** | Localhost-only HTTP, session token auth, dynamic port selection, SSE events |
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

# 3. Add concise repository knowledge (see CODESPACE.md in this repo), then build a Context Pack
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

## Desktop and dashboard

Launch the desktop window:

```bash
cse desktop
```

Or run the embedded web UI without opening a window:

```bash
cse serve --dashboard --port 8080
```

The interface provides:
- **Context Studio** — inspect Markdown knowledge and exact source evidence before inference
- **Pack telemetry** — stable pack ID, graph revision, token use, estimated savings, warnings, and provenance
- **Graph canvas** — interactive symbol relationship visualization
- **Symbol inspector** — click any node to see details
- **Live search** — real-time symbol lookup
- **Workspace selector** — switch between registered projects
- **Event stream** — SSE-powered live updates

The server binds to **localhost only**. The first local page response installs an **HttpOnly, SameSite=Strict** session cookie; the credential is not embedded in HTML, JavaScript, or the URL. If the requested port is busy, CodeSpace finds a free one and desktop mode opens the selected port.

## Markdown Context Cells

Context Cells live in normal indexed `.md`/`.mdx` files and are reviewed with the code they describe:

```markdown
<!-- cse:cell
id: auth.session-boundary
title: Local authentication contract
kind: constraint
status: accepted
priority: critical
tags: auth, security
paths: src/server.rs
symbols: check_authorization
-->
The control plane is loopback-only and browser auth uses an HttpOnly cookie.
<!-- /cse:cell -->
```

Cells are treated as repository evidence, never as higher-priority instructions. See [`CODESPACE.md`](CODESPACE.md) for the live repository brain and [`docs/CONTEXT_CELLS.md`](docs/CONTEXT_CELLS.md) for the protocol.

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
- **Fail-closed entropy** — the dashboard refuses to start if the OS cannot provide cryptographically secure randomness
- **Browser isolation** — HttpOnly/SameSite cookie auth, strict Host/Origin validation, CSP, frame denial, and no permissive CORS
- **Context trust boundary** — Markdown cells are labeled as evidence; unsafe evidence paths and duplicate IDs are rejected or warned
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
  context.rs        deterministic Context Pack compiler and token telemetry
  context_cells.rs  safe Markdown cell parsing, validation, and ranking
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
