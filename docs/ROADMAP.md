# Roadmap and Acceptance Criteria

## Release 1.0 — included

- Incremental local graph index.
- Multi-language structural extraction.
- Ranked compact context with secret redaction.
- Git blast-radius analysis.
- Decision memory.
- CLI, MCP, REST, library.
- JSON, DOT, HTML export.
- CI, release automation, security documentation, self-test harness.

## Release 1.1 — AST precision

### Critical

**Tree-sitter adapter**

Acceptance criteria:

- Rust, Python, JavaScript, TypeScript, Go, Java, C, and C++ AST tests cover at least 95% of declaration fixtures.
- Incremental parser updates preserve stable symbol IDs for unchanged declarations.
- Malformed files do not crash indexing.
- Parser dependencies and grammar licenses are recorded in SBOM/NOTICE.

**SQLite/WAL storage**

Acceptance criteria:

- schema migrations are reversible or have a tested forward-only recovery plan;
- concurrent readers never observe partial updates;
- crash injection proves index recovery;
- FTS queries match or exceed current ranking recall.

## Release 1.2 — semantic resolution

### Critical

**LSP resolver**

Acceptance criteria:

- process lifecycle, timeout, cancellation, and stderr capture are tested;
- definition/reference resolution has per-language accuracy reports;
- untrusted repositories cannot inject arbitrary LSP commands;
- fallback mode remains fully operational.

### Important

- inheritance/implementation edges;
- test-to-production mapping;
- rename preview and refactoring plans;
- symbol-level Git history.

## Release 1.3 — enterprise operation

### Critical

- authenticated HTTPS API via external gateway or dedicated server crate;
- tenancy and repository isolation;
- audit events and retention controls;
- configurable DLP policies;
- signed releases, provenance, SBOM, vulnerability scanning.

## Resource estimate

| Workstream | Skills | Indicative effort |
|---|---|---:|
| Tree-sitter languages | 2 Rust/static-analysis engineers | 8–12 engineer-weeks |
| LSP resolution | 2 Rust/IDE engineers | 10–16 engineer-weeks |
| SQLite/FTS/migrations | 1 Rust/data engineer | 4–6 engineer-weeks |
| Benchmark corpus | 1 evaluation engineer + language reviewers | 6–10 engineer-weeks |
| Secure remote service | 1 backend + 1 security engineer | 8–12 engineer-weeks |
| Packaging/release | 1 release engineer | 3–5 engineer-weeks |

## Risk register

| Priority | Risk | Impact | Mitigation |
|---|---|---|---|
| Critical | Brand/package collision | Takedown, confusion, install failure | Rename before public launch; trademark and registry clearance |
| Critical | False semantic edges | Unsafe refactoring advice | Confidence scores, source evidence, AST/LSP adapters, labeled evaluation |
| Critical | Secret leakage | Credential compromise | Exclusions, redaction, DLP tests, never index credential stores |
| Critical | Unauthenticated REST exposure | Repository disclosure | Loopback default, explicit remote opt-in, gateway requirement |
| Important | MCP protocol drift | Client incompatibility | Protocol conformance suite and official SDK adapter |
| Important | Large monorepo performance | Poor adoption | SQLite/FTS, incremental ASTs, sharding, benchmark gates |
| Important | License contamination | Legal exposure | Clean-room policy, SBOM, commit-level provenance review |
| Optional | HTML graph scaling | Browser slowdown | Progressive loading/WebGL exporter |
