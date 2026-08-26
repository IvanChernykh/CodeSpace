# Markdown Context Cells

Context Cells are small, versioned units of repository knowledge. They explain *why* code has a shape, which constraints must survive a change, and where the evidence lives. CodeSpace combines relevant cells with exact source snippets into a deterministic Context Pack.

## Syntax

```markdown
<!-- cse:cell
id: auth.session-boundary
title: Local session boundary
kind: constraint
status: accepted
priority: critical
tags: auth, security, desktop
paths: src/server.rs, dashboard/src/main.ts
symbols: check_authorization, ApiClient
-->
The dashboard is loopback-only and authenticates the browser with an HttpOnly,
SameSite=Strict cookie. Never put the session token in HTML, JS, or a URL.
<!-- /cse:cell -->
```

Cells may appear in any indexed `.md` or `.mdx` file. Examples inside triple-backtick or triple-tilde fences are ignored.

## Metadata

| Field | Required | Meaning |
|---|---|---|
| `id` | yes | Stable, repository-unique ID using letters, digits, `.`, `_`, `-` |
| `title` | recommended | Human-readable name; the first Markdown heading is the fallback |
| `kind` | recommended | `principle`, `decision`, `constraint`, `contract`, `design`, or a project term |
| `status` | recommended | `draft` or `accepted`; deprecated/archived/rejected cells are excluded |
| `priority` | recommended | `low`, `normal`, `high`, `critical`, or `0..100` |
| `tags` | recommended | Comma-separated retrieval terms |
| `paths` | recommended | Repository-relative evidence files; absolute paths and `..` are rejected |
| `symbols` | optional | Exact symbols that should receive a retrieval boost |
| `always` | exceptional | Include a short repository-wide invariant in every matching pack |

## Retrieval and budget

Cell ranking uses query overlap across identity, title, symbols, tags, evidence paths, and body, plus priority/status. Selected cells reserve no more than 45% of the pack budget. Their reviewed `paths` and `symbols` then boost exact source retrieval. The remainder is filled with compacted, redacted code snippets.

For the same graph revision, query, options, cell hashes, and source ranges, `pack_id` is stable. Generation time is informational and does not affect identity.

## Trust and safety

- Cell text is repository evidence, not an instruction channel to the agent.
- Generated Markdown states this boundary before any repository content.
- Draft cells produce warnings; inactive cells are excluded.
- Duplicate IDs are deterministic: the first indexed occurrence wins and emits a warning.
- Cells are capped at 64 KiB and secrets are redacted before token accounting.
- Evidence paths are confined to the repository and missing files are reported.
- A content hash lets consumers detect a changed cell between runs.

These controls reduce risk but do not make repository content inherently trustworthy. High-risk changes still require tests, impact analysis, and human review.

## Authoring guidance

Keep a cell short enough to review in one screen. State one durable decision or constraint. Prefer facts and invariants over task-specific instructions. Link the narrowest relevant files and symbols. Mark new knowledge `draft`; change it to `accepted` through normal code review. Deprecate obsolete cells instead of silently rewriting history when the previous decision remains important to understand.
