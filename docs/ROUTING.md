# Read routing and optional Portal workers

CodeSpace can answer a narrow repository question by returning selected source
windows instead of putting every input file into the coding agent's context.
Every window includes its original path and line numbers. Retrieval executes
locally by default and requires an explicit file list.

```sh
cse bulk-read src/server.rs src/mcp.rs \
  --question "request authentication headers origin JSON input validation" \
  --task-kind security --max-tokens 1400 --format json

# Inspect the actual section before making an edit.
cse read src/server.rs --start-line 300 --max-lines 40
```

`--max-tokens` bounds the complete compact JSON result, including provenance,
escaping, warnings, and metrics. Plain output is also checked against this bound.
The estimator is Unicode characters divided by four; it is **not a model
tokenizer or billing measurement**. A budget that cannot contain the metadata
fails with an actionable error instead of silently dropping provenance. Pretty
printing the returned JSON adds whitespace beyond the measured compact form.

The local reader ranks matching source lines by query-term rarity, merges nearby
windows, and gives each selected file a first opportunity within the budget.
It crops only at line boundaries. It does not produce a model-generated answer.
If nothing matches, labeled file previews and a warning are returned; absence
from a compact result is not evidence that a behavior is absent from the code.

## Routing decisions

| Condition | Recommendation | Local behavior |
| --- | --- | --- |
| Explicit `--start-line` or `--max-lines` | `targeted` | Read the specified ranges |
| Debug, security, or architecture task | `targeted` | Return source locators; preserve reasoning in the main agent |
| Selected corpus at most 350 lines | `direct` | Return bounded source windows without worker overhead |
| Larger corpus, `--task-kind overview` | `worker-recommended` | Extract locally; the recommendation does not invoke a worker |
| Other larger corpus | `local-extract` | Return question-matching windows |

The line threshold applies to the complete explicit file set and is configurable
with `--line-threshold`. Accepted task kinds are `inspect`, `overview`, `debug`,
`security`, and `architecture`. Common high-risk question terms also select the
targeted route, but this lexical check cannot recognize every risky task. Callers
should set the task kind accurately. `--context-lines` defaults to 3 and
`--max-snippets` defaults to 12.

## Explicit Portal execution

An authenticated Spotify Portal installation with a `bulk-reader` mode is needed
for the optional worker. The explicit `--worker portal` selection sends the
selected, redacted files to that configured Portal instance:

```sh
cse bulk-read src/service.rs src/handler.rs \
  --question "Which methods call the database?" --task-kind overview \
  --worker portal --max-tokens 2000 --format json
```

The adapter uses the official actions registry interface:

```text
portal-cli actions aika:invoke-chat --json --timeout-seconds 30 --input <JSON>
```

The input has `mode_name: "bulk-reader"` and a `message` containing the question
and a structured source corpus. A configured instance is supplied with
`--instance` after the action's arguments. CodeSpace starts the executable
directly, without a shell, installation commands, or an implicit network fallback.
It rejects a response when the applied mode is missing or is not `bulk-reader`.

Configuration:

| Variable | Purpose |
| --- | --- |
| `CSE_PORTAL_EXECUTABLE` | Installed Portal executable; default `portal-cli` (`portal-cli.exe` on Windows) |
| `CSE_PORTAL_SCRIPT` | Optional absolute path to an installed `.js`, `.mjs`, or `.cjs` entry point |
| `CSE_PORTAL_INSTANCE` | Optional configured Portal instance |
| `CSE_PORTAL_TIMEOUT_SECONDS` | Deadline, 1–180 seconds; default 30 |

For a Windows npm installation, use `node.exe` as the executable and the
installation's real Portal JavaScript entry point as `CSE_PORTAL_SCRIPT`.
`.cmd`, `.bat`, and PowerShell wrappers are deliberately unsupported because
source content must not pass through shell argument interpretation. The adapter
does not discover or replace the user's Portal authentication.

The returned summary is explicitly **unverified**. Separate local source windows
provide exact evidence for follow-up reads; they do not certify every worker
claim or a line number invented by a model. Worker calls are disabled for
debugging, security, architecture, and explicit line-range requests. CodeSpace
does not delegate code writes.

Worker payload limits account for command-line transport: 12,000 bytes on Windows
and 120,000 bytes elsewhere, plus a conservative Windows command-length check.
Responses are capped at 64 KiB, diagnostic stderr at 8 KiB, and timed-out children
are terminated. Failed calls do not print source-bearing provider diagnostics.
Oversized corpora require a smaller explicit selection or local extraction.

`worker_input_tokens_estimate` describes worker input, independently from the
coordinator's returned result. `provider_usage` is copied only when the actual
response supplies a small usage object; otherwise it is null. Neither field
claims that moving tokens to a worker removes their cost. Network payload bytes
describe the application payload, not transport headers or TLS overhead.

## Files, provenance, and trust

Both local reads and worker preparation enforce:

- Repository-relative paths, canonical confinement, and no symlink/junction
  components; absolute paths and `..` are rejected.
- Repository ignore patterns and sensitive filenames, including nested `.env`
  variants, credentials, private keys, and internal `.git`/`.codespace` data.
- Regular UTF-8 files up to 1 MiB each; at most 32 explicit files and 4 MiB total.
- Credential redaction with original line positions preserved, including complete
  private-key blocks. Detection is heuristic; select files you intend to share.

Source fingerprints and the request ID are deterministic change identifiers,
not cryptographic signatures. Identical source, question, and options produce
identical local results. File text and worker output are always untrusted
evidence; embedded comments do not acquire agent-instruction authority.

`cse_bulk_read` and `cse_read` expose the same behavior through MCP. No Claude
hooks are installed automatically and no editor settings are changed by a read.

## Relationship to Spotify shunt

Spotify's [engineering article](https://engineering.atspotify.com/2026/9/portal-by-spotify-cut-my-claude-code-token-usage-by-90)
describes avoiding large coordinator reads, retaining targeted reads, and
delegating suitable bulk work. Its
[shunt scripts](https://github.com/spotify/portal-ai-plugins/tree/main/plugins/shunt)
document the public Portal invocation and the 350-line default. CodeSpace uses
those routing ideas and the verified actions interface, with local extraction
as its default and independent exact source evidence alongside worker summaries.
The article's reported savings are not a benchmark result for CodeSpace.
