# CodeSpace breakthrough roadmap

CodeSpace will not win by cloning Git commands or adding more dashboard panels. It must make repositories dramatically cheaper for AI to understand and materially safer for humans to change, while remaining compatible with Git hosting, editors, and CI.

## Product scorecard

Every release is gated by five outcomes:

1. **Correctness:** task success with CodeSpace context is non-inferior to full-repository baselines.
2. **Token efficiency:** median model input falls by at least 60% on the published benchmark corpus.
3. **Provenance:** every retrieved fact points to a cell or exact file/line range and a stable hash.
4. **Safety:** secrets, path escapes, prompt-like repository text, and unsafe remote access fail closed.
5. **Time to useful context:** warm p95 pack compilation stays below 150 ms on a 100k-file monorepo target.

## 2.1 — Context Packs and secure desktop foundation

Status: vertical slice implemented in this repository.

- Versioned Markdown Context Cells with IDs, kinds, statuses, priorities, tags, paths, and symbols.
- Deterministic hybrid packs combining cells and exact code under a hard token budget.
- Pack ID, graph revision, cell hashes, source provenance, redaction warnings, and estimated token savings.
- Explicit evidence/instruction trust boundary for AI consumers.
- Context Studio UI with knowledge/code split and copyable JSON.
- `cse desktop` app-window launcher over the secured loopback control plane.
- CSPRNG sessions that fail closed, HttpOnly cookie auth, Host/Origin validation, CSP, rate/connection limits.

Exit criteria:

- all Rust/CLI/dashboard/smoke suites pass on Windows, Linux, and macOS;
- a 50-task benchmark publishes raw token, latency, correctness, and tool-call results;
- Context Cell parser has fuzz/property coverage for malformed Markdown and Unicode;
- no auth credential appears in HTML, JS, query strings, screenshots, or process arguments.

## 2.2 — Production knowledge index

- SQLite/WAL storage with transactional migrations and FTS5/BM25 hybrid retrieval.
- Tree-sitter adapters for Rust, Python, TypeScript/JavaScript, Go, Java, C, and C++.
- Incremental file watching with revision snapshots and cancellation.
- Cell linter/editor, stale-evidence detection, ownership, review signatures, and schema migration.
- Pluggable tokenizer accounting for the selected model instead of the conservative 4-char estimate.
- LSP/IDE extensions that request Context Packs automatically and show provenance inline.

Exit criteria:

- crash injection never exposes partial index state;
- unchanged symbols keep stable IDs across incremental AST rebuilds;
- retrieval recall improves over 2.1 on the labeled corpus without increasing median tokens;
- database corruption and downgrade paths have tested recovery procedures.

## 3.0 — Semantic change system

This is the point where CodeSpace becomes more capable than Git for AI-assisted change preparation while still synchronizing through Git.

- Content-addressed **Change Intents** containing objective, constraints, Context Pack, proposed symbol edits, tests, impact proof, and approvals.
- Symbol-aware diff and three-way semantic merge with exact textual fallback.
- Pre-commit policy engine: required tests, secret/DLP checks, ownership, risk thresholds, and reproducibility proof.
- Signed context/change manifests and tamper-evident local audit log.
- Lossless Git import/export, patch generation, branch/PR adapters, and offline bundles.
- Desktop review flow that explains why each change exists and which context justified it.

Exit criteria:

- every semantic operation round-trips to a byte-identical Git worktree;
- merge evaluation reduces unresolved conflicts by at least 30% without increasing silent mismerges;
- a Change Intent can be reproduced offline from its object graph and verified without trusting the author;
- destructive operations always present an impact proof and recoverable rollback path.

## 3.1 — Team and enterprise operation

- Optional encrypted collaboration relay; local/offline remains first-class.
- Repository/tenant isolation, SSO through an external gateway, audit export, retention, and configurable DLP.
- Signed releases, SLSA provenance, SBOM, vulnerability scanning, reproducible binaries, and automatic rollback.
- Shared cell review/ownership and policy packs pinned by content hash.

Remote operation is not enabled by extending the local server. It is a separate threat model, deployment profile, and security review.

## CI/CD gates

- PR: format, clippy (`correctness` + selected `suspicious`), all tests, dashboard build/contract, smoke test, MSRV, license/deny, and secret scan.
- Nightly: fuzzing, mutation/property suites, benchmark regression, dependency audit, and cross-platform desktop launch smoke.
- Release: clean-tree rebuild, SBOM, provenance attestation, signature, checksum, installer smoke, rollback rehearsal, and canary channel.

## Risk register

| Priority | Risk | Mitigation |
|---|---|---|
| Critical | Context Cell prompt injection | Evidence boundary, status/review metadata, safe rendering, adversarial benchmark |
| Critical | False semantic merge | Exact fallback, confidence threshold, mandatory tests, round-trip proof |
| Critical | Secret leakage | exclusions, redaction, fail-closed DLP gates, never index credential stores |
| Critical | Repository corruption | immutable objects, transactional writes, fsync, backups, recovery drills |
| Important | Misleading token-savings claims | publish corpus, baseline definition, raw trials, model-specific tokenizers |
| Important | Git ecosystem incompatibility | lossless import/export and Git as transport until parity is proven |
| Important | Desktop attack surface | loopback isolation, HttpOnly session, strict CSP/origin, signed shell |
| Important | Large monorepo latency | incremental ASTs, WAL/FTS, sharding, p95 performance gates |
