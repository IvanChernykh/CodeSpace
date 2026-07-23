# Changelog

## 2.0.0 — 2026-07-23

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
