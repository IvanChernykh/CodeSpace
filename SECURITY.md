# Security Policy

## Supported version

| Version | Supported |
|---|---|
| 1.0.x | Yes |

## Threat model

CodeSpace processes untrusted repository content. The primary risks are accidental secret disclosure, path traversal, resource exhaustion, index corruption, malicious generated files, and unauthenticated network exposure.

## Controls

### Critical

1. **Local-first:** the binary contains no outbound network client.
2. **Path confinement:** MCP reads canonicalize the requested path and reject paths outside the project root.
3. **Secret filtering:** context and MCP reads redact common API-token/private-key patterns and credential assignments.
4. **Resource limits:** indexing defaults to 1 MiB per file; MCP file reads are capped at 2 MiB; probable binary files are excluded.
5. **Symlink policy:** symlinks are ignored by default.
6. **Crash-aware storage:** the index is written to a temporary file and synchronized; POSIX uses atomic rename, while Windows uses a recoverable backup swap.
7. **REST exposure:** default bind address is loopback. Non-loopback bind requires explicit `--allow-remote`.

### Important

- `.git`, `.codespace`, dependency, build, coverage, and common vendor directories are ignored.
- MCP logs are written only to stderr; stdout is reserved for protocol messages.
- No `unsafe` Rust is allowed by crate lint.
- The on-disk format validates record counts, numeric fields, and enum values.

## Residual risks

- Secret detection is heuristic and can have false negatives.
- Structural parsing is not a compiler and can produce incorrect edges.
- REST has no authentication, authorization, TLS, rate limiting, or CSRF model.
- File watcher polling can miss short-lived transient states, although the next stable state is detected.
- Git diff analysis depends on local Git object integrity and available refs.

## Deployment requirements

Do not expose REST directly to an untrusted network. Use a reverse proxy with TLS, authentication, authorization, request-size limits, and audit logging. Run with least filesystem privilege. Exclude credential directories explicitly. Keep the repository and `.codespace` directory on a trusted filesystem.

## Reporting

Do not open a public issue for a suspected vulnerability. Contact the maintainers privately and include affected version, reproducible steps, impact, and proposed mitigation.
