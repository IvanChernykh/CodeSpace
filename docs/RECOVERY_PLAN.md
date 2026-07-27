# CodeSpace Dashboard Recovery Plan

Status: **active recovery**  
Branch: `recovery/dashboard-stability`

## Why this branch exists

The dashboard accumulated UI changes faster than its runtime contract was verified. A build or a visually improved screenshot is not evidence that the installed application works. This branch introduces release gates before another redesign is merged.

## Non-negotiable architecture

1. **One application service layer.** CLI, localhost API, dashboard and MCP must call the same actions. A feature is incomplete until all required surfaces use the same implementation.
2. **Fresh binary only.** CI and installation checks must execute the binary built from the tested commit. PATH lookup is prohibited in acceptance scripts.
3. **Localhost security.** The server binds to `127.0.0.1`; static bootstrap assets may be public, while repository data and mutations require the session token.
4. **Explicit degraded states.** Missing Ollama, GitHub credentials, MCP process or index must produce actionable UI states, never infinite spinners or fake `Live` indicators.
5. **No generated-source drift.** TypeScript compilation must leave `dashboard/dist` clean in git.

## Product information architecture

The dashboard is a developer control center, not a collection of unrelated tabs.

### Primary navigation

- **Overview** — active workspace, index health, pending tasks, AI/MCP/GitHub status and recent activity.
- **Code Graph** — file-first dependency graph, symbol drill-down, search, context and impact.
- **Assistant** — local AI chat with visible model/runtime state and optional repository context.
- **Work** — task board and calendar backed by one task model.
- **Integrations** — MCP servers, GitHub connection and skills.
- **Settings** — workspace/global settings, privacy, model and appearance.

### Layout rules

- Persistent left navigation; workspace switcher is always visible.
- Main content uses one dominant task per screen.
- Inspector is contextual and collapsible, not permanently consuming space.
- Empty, loading, degraded and error states are designed components.
- Desktop target: 1280×720 and above. Compact mode: 900–1279 px. Below 900 px uses drawers instead of three fixed columns.

## Functional parity matrix

Every action must declare its supported surfaces and tests:

| Capability | CLI | API/UI | MCP | Required test |
|---|---:|---:|---:|---|
| Index/update | yes | yes | yes | same revision and counts |
| Search | yes | yes | yes | same ordered symbol IDs |
| Context | yes | yes | yes | same selected files/symbols |
| Impact | yes | yes | yes | same affected nodes |
| Read | yes | yes | yes | path confinement + redaction |
| Decisions/history | yes | yes | yes | create/read round-trip |
| Workspaces | yes | yes | optional | register/select/remove |
| Tasks | yes | yes | yes | create/update/delete/list |
| AI chat | yes | yes | yes | degraded + configured runtime |
| GitHub | yes | yes | yes | disconnected + connected states |
| Skills | yes | yes | yes | permission and enable/disable |
| MCP manager | yes | yes | n/a | lifecycle and health states |

## Release gates

A dashboard release is blocked unless all gates pass:

1. `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --locked`.
2. Dashboard TypeScript build succeeds and `dashboard/dist` has no diff.
3. `scripts/dashboard_smoke.py` passes on Windows and Linux.
4. Static assets return `200` without authorization and correct content types.
5. Protected repository APIs return `401` without a token and `200` with a token.
6. Browser bootstrap contains the session token, JS and CSS entry points.
7. Tasks pass create/list/update/delete persistence tests.
8. SSE reports connected only after the stream opens; disconnects become visible within the retry interval.
9. AI, GitHub and MCP missing dependencies show a degraded state with a repair action.
10. The installer records the exact binary path and verifies `cse --version` from that path.

## Recovery sequence

### R0 — Stabilize

- Add executable E2E smoke coverage.
- Stop merging dashboard changes directly to `main`.
- Remove false status indicators and infinite loading states.
- Make server errors structured and visible.

### R1 — Contract consolidation

- Inventory CLI/API/MCP actions.
- Route tasks, AI, GitHub and skills through the application registry.
- Introduce typed request/response schemas and parity tests.

### R2 — UX reconstruction

- Replace the horizontal tab dump with the product information architecture above.
- Build Overview, Code Graph and Assistant first.
- Add responsive layout, keyboard navigation and accessible focus states.

### R3 — Integrations

- Implement MCP lifecycle controls and logs.
- Implement GitHub OAuth/device-flow or explicit token configuration without exposing tokens to the browser.
- Implement signed/pinned skill installation with license and permission review.

### R4 — Packaging

- One install location per platform.
- `cse doctor` reports executable path, version, config path, index, dashboard, AI and MCP health.
- Release artifact runs the same E2E suite before publication.

## Definition of done

A feature is not done because its module, endpoint or visual panel exists. It is done only when:

- the user can complete the workflow from the dashboard;
- the same operation is available through the declared CLI/MCP surfaces;
- state survives restart when persistence is expected;
- failure and disconnected states are understandable;
- automated acceptance tests prove the workflow against the built binary.
