# Architecture

## Design priorities

1. Deterministic, local, bounded execution.
2. Useful context with explicit confidence rather than fabricated semantic certainty.
3. One binary and no runtime dependency.
4. Stable interfaces across CLI, MCP, REST, and library.
5. Upgrade path to parser/LSP/database adapters without changing the product contract.

## Data flow

```text
Repository traversal
  -> ignore/symlink/size/binary policy
  -> language detector
  -> declarations/imports/calls extractor
  -> symbol and edge normalization
  -> deterministic graph
  -> crash-aware index replacement

Task query
  -> tokenization
  -> exact/prefix/path/signature scoring
  -> one-hop graph propagation
  -> source range selection
  -> compaction
  -> secret redaction
  -> token budget
  -> CLI/MCP/REST response

Git refs
  -> unified diff hunks
  -> changed source lines
  -> intersecting symbols
  -> reverse graph traversal
  -> risk score and warnings
```

## Persistence format

`.codespace/index.csf` is a versioned escaped-TSV event snapshot:

- `META`
- `FILE`
- `SYMBOL`
- `EDGE`
- `DECISION`

The format is intentionally inspectable and deterministic. A production SQLite adapter should preserve the same domain model and add WAL, FTS5, migrations, and transactional multi-reader behavior.

## Consistency

- IDs are stable FNV-1a hashes of normalized identities.
- Files are replaced as units.
- Cross-file edges are re-resolved after changes to avoid stale call/import links.
- Writes are serialized by an exclusive lock and committed by atomic rename on POSIX and a recoverable backup swap on Windows.
- Search ordering is score-first and stable-ID second, guaranteeing repeatable output.

## Parser precision

The 1.0 parser is structural and language-aware. It extracts useful declarations and high-confidence local calls without executing code. It does not claim macro expansion, dynamic dispatch resolution, type inference, conditional compilation evaluation, or complete overload resolution.

The precision roadmap introduces a `ParserAdapter` and `ResolverAdapter` boundary:

```text
HeuristicParser (built-in fallback)
TreeSitterParser (AST extraction)
LspResolver (definitions/references/rename)
CompilerResolver (language-specific optional integration)
```

## MCP

The server uses newline-delimited JSON-RPC over stdio, supports lifecycle initialization, `tools/list`, and `tools/call`, and keeps stdout protocol-clean. The five-tool surface is intentionally small to limit schema/context overhead.

## REST

The REST server is an operational convenience, not an internet service. It is read-only, loopback by default, and intentionally minimal. Each request reloads the persisted index so long-running processes observe successful `cse update` writes. Production remote operation requires a separate authenticated gateway.
