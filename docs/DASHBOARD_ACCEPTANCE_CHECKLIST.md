# Dashboard acceptance checklist

Use this checklist for every dashboard or server pull request.

## Build identity

- [ ] The tested binary is built from the pull request commit.
- [ ] The test invokes the binary by absolute path, not through `PATH`.
- [ ] `cse --version` matches the package version.
- [ ] Generated dashboard assets match their TypeScript sources.

## Startup and shutdown

- [ ] An isolated repository can be initialized.
- [ ] The server binds only to `127.0.0.1`.
- [ ] A requested free port is used without silent reassignment.
- [ ] The session token is emitted once and injected into dashboard bootstrap.
- [ ] The process terminates cleanly.

## Browser bootstrap

- [ ] `/` returns HTML.
- [ ] `/assets/main.js` returns JavaScript without authentication.
- [ ] `/assets/dashboard.css` returns CSS without authentication.
- [ ] Missing assets return `404`, not dashboard HTML.
- [ ] The initial page never remains in an infinite loading state.

## Security

- [ ] Repository APIs reject unauthenticated requests.
- [ ] A valid Bearer token authorizes protected APIs.
- [ ] Paths cannot escape the active workspace.
- [ ] Secrets are redacted from read/context/AI payloads.
- [ ] GitHub and model credentials are never embedded in browser HTML or JS.

## Core workflows

- [ ] Graph loads and contains indexed files or symbols.
- [ ] Search selects a result and focuses the graph/inspector.
- [ ] Context can be generated and copied.
- [ ] Impact analysis explains affected symbols/files.
- [ ] Workspace register/select/remove survives restart.
- [ ] Task create/update/delete/list survives restart.
- [ ] AI chat shows configured, unavailable and error states.
- [ ] GitHub shows disconnected, connected and API-error states.
- [ ] Skills show permissions before enable/install.
- [ ] MCP manager shows process status, tools, logs and restart controls.

## UX quality

- [ ] The active workspace and system health are visible without opening settings.
- [ ] Empty states explain the next action.
- [ ] Errors identify the failed subsystem and recovery action.
- [ ] Keyboard focus is visible.
- [ ] Main workflows work at 1280×720.
- [ ] Layout remains usable between 900 and 1279 px.
- [ ] Narrow view uses drawers rather than crushed fixed sidebars.
- [ ] Status text is factual; `Live` is never shown from health polling alone when realtime events are disconnected.

## CI evidence

- [ ] Rust formatting, lint and tests pass.
- [ ] Dashboard TypeScript build passes.
- [ ] `scripts/dashboard_smoke.py` passes on Windows.
- [ ] `scripts/dashboard_smoke.py` passes on Linux.
- [ ] The pull request includes screenshots or recordings for visual changes.
