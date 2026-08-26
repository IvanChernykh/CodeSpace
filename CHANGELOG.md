# Changelog

## 2.0.0 — 2026-08-26

### Repository Brain

- Added typed Markdown Context Cells in `CODESPACE.md` for durable principles,
  constraints, decisions, and design evidence kept next to the code.
- Added deterministic Context Packs that combine ranked knowledge cells and exact
  source snippets under one hard token budget.
- Every pack now exposes its graph revision, stable pack ID, source provenance,
  content hashes, redaction warnings, and an operational compression estimate.
- Cell evidence paths and symbols participate in source ranking, keeping the
  knowledge layer connected to inspectable code.

### Desktop and Context Studio

- Added `cse desktop`, which opens the secure loopback dashboard in an app window
  where the operating system supports it.
- Rebuilt Context Studio around query composition, budget visibility, knowledge
  and source layers, provenance, warnings, and JSON export.
- Added responsive desktop-grade styling and production dashboard contract tests.

### Security

- Dashboard sessions now use an HttpOnly, SameSite=Strict cookie; credentials are
  absent from HTML, JavaScript state, query strings, browser arguments, and logs.
- Secure session generation fails closed if operating-system entropy is
  unavailable.
- Added loopback Host and Origin validation, connection limits, authentication
  rate limiting, strict response headers, and broader secret-redaction coverage.

### Platform foundation

- Transformed from CLI tool into a local IDE assistant platform.
- **Action Registry**: unified application core for CLI, UI, REST, and MCP.
- **Local Dashboard**: embedded web UI with interactive graph canvas, inspector, search, and workspace selector.
- **Workspace Manager**: multi-repository registration, selection, and per-workspace settings.
- **Local Daemon**: single-instance discovery, session/bootstrap security, WebSocket events, graceful shutdown.
- **MCP Management**: CodeSpace MCP server state plus external MCP server lifecycle management.
- **Skills Platform**: manifest-based skills with permission model, built-in catalog, and runtime.
- **Security Hardening**: localhost CSRF protection, path confinement, secret redaction, permission policies.
- **Event Bus**: realtime synchronization between CLI, UI, and MCP with state versions.
- **Versioned Index**: index revisions, atomic publication, stale detection.
- **Extended Graph Model**: new edge kinds (extends, test-covers, configures, depends-on), precision tiers, evidence.
- **API Versioning**: `/api/v1/` prefix with unified error envelope.
- **Settings System**: global, workspace, and session settings with priority chain.

### Upgrade notes and current limits

- Run `cse update --path <repository>` after upgrading to refresh the semantic
  index and discover Context Cells. Existing Git history is never rewritten.
- The built-in token and compression metrics use a deterministic four-character
  operational estimate. Published model-specific claims still require the exact
  tokenizer and benchmark protocol described in `docs/BENCHMARKS.md`.
- Content-addressed Change Intents, semantic merge, signatures, and native OS
  installers remain roadmap work; this release interoperates with Git instead of
  replacing its object database.

## 1.0.0 — 2026-07-23

- Final product identifiers: **CodeSpace**, binary `cse`, package `codespace-cse`, MCP tools `cse_*`, local state `.codespace/`.
- Added a dependency-free GitHub Pages site and automated Pages deployment.
- Initial local-first release.
- Incremental semantic graph index and watch mode.
- Multi-language structural parser.
- Ranked compact context with token budget and secret redaction.
- Git blast-radius analysis and risk scoring.
- Persistent engineering decision memory.
- CLI, MCP stdio, loopback REST, and Rust library interfaces.
- JSON, Graphviz, and standalone HTML exports.
- Doctor, stats, benchmark, shell, CI, release workflow, and security documentation.
