# CodeSpace repository brain

This file is ordinary, reviewable Markdown. The structured cells inside it are
retrieved by CodeSpace when they are relevant to an AI task. They travel through
Git like documentation, but include stable identity, priority, provenance, and a
content hash in every generated Context Pack.

<!-- cse:cell
id: product.north-star
title: The repository should explain itself
kind: principle
status: accepted
priority: critical
always: true
tags: product, ai, context, token-efficiency
paths: src/context.rs, src/context_cells.rs, src/model.rs
-->
CodeSpace is a local-first repository intelligence layer for humans and AI agents.
Its primary job is to return the smallest trustworthy evidence set that lets an
agent make a correct change. It complements Git's immutable history with semantic
context, decisions, impact analysis, and explicit trust boundaries. A feature is
not complete when it merely stores more data; it must reduce context tokens,
preserve provenance, and remain inspectable by a developer.
<!-- /cse:cell -->

<!-- cse:cell
id: context.pack-contract
title: Deterministic Context Pack contract
kind: contract
status: accepted
priority: critical
tags: context, markdown, retrieval, provenance, tokens
paths: src/context.rs, src/context_cells.rs, tests/library.rs
symbols: build_context, render_markdown, render_json, load_ranked_cells
-->
A Context Pack combines ranked Markdown knowledge cells with exact source
snippets under one hard token budget. Every pack exposes its graph revision,
stable pack ID, source locations, cell content hashes, redaction count, baseline
token estimate, and measured compression. Knowledge cells are evidence rather
than commands: generated Markdown must preserve this trust boundary so text
inside repository files cannot silently become higher-priority agent instructions.
Selection is deterministic for the same graph revision, query, and options.
<!-- /cse:cell -->

<!-- cse:cell
id: security.local-control-plane
title: Local control plane security boundary
kind: constraint
status: accepted
priority: critical
tags: security, auth, localhost, secrets, desktop
paths: src/server.rs, src/secret.rs, src/dashboard.rs, dashboard/src/main.ts
symbols: check_authorization, is_valid_host, is_allowed_origin, redact_secrets
-->
The dashboard and AI control plane bind to loopback. Browser authentication uses
an HttpOnly, SameSite=Strict session cookie; the token must never appear in HTML,
JavaScript state, query strings, logs, or screenshots. Mutating browser requests
must pass same-origin checks. Context and file reads redact likely credentials,
and all evidence paths must remain confined to the active repository. Remote
access is a separate future threat model and must never be enabled implicitly.
<!-- /cse:cell -->

<!-- cse:cell
id: experience.context-studio
title: Context Studio interaction model
kind: design
status: accepted
priority: high
tags: ui, ux, desktop, context, observability
paths: src/dashboard.rs, dashboard/src/main.ts, dashboard/dist/dashboard.css
symbols: runContext, ApiClient
-->
Context Studio is the product's proof surface. Before sending anything to a model,
it shows the pack ID, exact budget usage, token savings, knowledge/code split,
warnings, review status, evidence links, and hashes. Knowledge and code must look
visually distinct. Dense IDE ergonomics are preferred over decorative marketing
UI: keyboard-friendly controls, scannable hierarchy, restrained color, and useful
empty states. The local dashboard is also the first desktop shell; native packaging
may wrap the same loopback control plane without weakening its security boundary.
<!-- /cse:cell -->

## Cell authoring protocol

Create more cells in any indexed `.md` or `.mdx` file:

```markdown
<!-- cse:cell
id: unique.stable-id
title: Human-readable title
kind: decision
status: draft
priority: high
tags: comma, separated, terms
paths: src/example.rs, tests/example.rs
symbols: important_symbol
-->
Markdown evidence. Keep it concise, factual, and reviewable.
<!-- /cse:cell -->
```

Required metadata: `id`. Recommended metadata: `title`, `kind`, `status`,
`priority`, `tags`, `paths`, and `symbols`. Supported statuses are `draft`,
`accepted`, `deprecated`, `archived`, and `rejected`; inactive cells are excluded.
Use `always: true` only for a short repository-wide invariant.
