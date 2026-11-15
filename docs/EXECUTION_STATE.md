# Execution checkpoints

CodeSpace keeps a bounded checkpoint for an agent task in
`.codespace/runs/<id>.json`. A checkpoint records the current objective, facts,
work phase, completed outcomes, and proposed next action. It can be handed to a
new agent session without replaying the previous session's tool output.

This design takes the explicit state transition idea from
[SKILL.state, sections 3.1–3.3](https://arxiv.org/html/2608.26263). It is an
implementation of bounded state storage and handoff, not a reproduction of that
paper's evaluation. CodeSpace does not control an external agent's conversation
buffer or automatically discard that agent's history.

## Contract

The Rust API uses `serde_json::Value` at the boundary:

```rust,ignore
execution_state::start(root, "release-check", &initial)?;
execution_state::read(root, "release-check")?;
execution_state::patch(root, "release-check", expected_revision, &changes)?;
execution_state::resume(root, "release-check")?;
execution_state::list(root)?;
```

`start` requires an objective and supplies an initial specification, active
status, empty lists, inspection phase, and proposed first action. The caller
can supply its own specification at creation. The objective and specification
cannot be changed afterward. Changing the task means starting another run.

The persisted envelope contains `schema_version`, `id`, `revision`,
`created_unix_ms`, `updated_unix_ms`, and `state`. Revisions begin at zero.

| State field | Type | Limit | Patch semantics |
| --- | --- | --- | --- |
| `objective` | nonempty string | 4,096 UTF-8 bytes | Immutable |
| `specification` | nonempty string | 4,096 bytes | Immutable |
| `status` | string enum | `active`, `blocked`, `paused`, `completed`, `cancelled` | Replace; cannot delete |
| `phase` | string | 128 bytes | Replace or delete |
| `constraints` | string array | 16 entries, 512 bytes each | Replace whole array or delete |
| `active_files` | relative-path array | 32 entries, 256 bytes each | Replace whole array or delete |
| `facts` | string array | 32 entries, 768 bytes each | Replace whole array or delete |
| `completed` | string array | 16 entries, 512 bytes each | Replace whole array or delete |
| `next_action` | string | 2,048 bytes | Replace or delete |
| `latest_observation` | string | 4,096 bytes | Replace or delete |

The complete serialized envelope must fit in **16,384 bytes**, including JSON
escaping and metadata. Individual field limits do not override that total.
The handoff prompt must fit in **18,432 bytes**. These are byte limits, not model
token counts. Run length does not increase either limit. The test suite verifies
500 replacements without accumulating earlier facts or observations.

Completed and cancelled runs are terminal. Their state remains readable, but
all patches fail. `resume` constructs a handoff prompt; it does not reopen a
terminal run, start an agent, or execute `next_action`.

## Updating state

Submit a top-level object containing only fields being changed:

```json
{
  "phase": "verify",
  "active_files": ["src/context.rs", "tests/library.rs"],
  "facts": ["The hard-budget regression passes on the current checkout."],
  "completed": ["Validated total context budget enforcement."],
  "latest_observation": "cargo test: budget regression passed",
  "next_action": "Run the full required test suite."
}
```

The caller must supply the revision it read. A stale revision fails without
writing anything. Read the latest state, reconcile the facts, then retry with
that revision. Do not blindly replay a stale update.

Patches replace a field rather than recursively merging it. `null` deletes an
optional field; an empty array explicitly records an empty set. Unknown keys,
wrong types, oversized values, unsafe paths, and invalid status values fail
before the checkpoint is replaced. There is no `history`, `messages`, or
`reasoning` field. Keep concise facts and verified outcomes in the accepted
fields; the schema cannot determine whether arbitrary prose is appropriate.

When a field reaches its limit, replace obsolete information with the current
facts needed for the next action. Preserve durable project knowledge in a
reviewed Markdown Context Cell, then reference the relevant file in the state.

## Persistence and recovery

Writers acquire a per-run OS advisory lock. The lock is released when its file
handle closes or the process exits. The empty `.lock` file remains on disk;
its presence does not mean a writer is still running. Do not remove lock files
while CodeSpace processes are active. Writers wait at most three seconds for a
lock, then return a retryable error.

A write validates the complete proposed state, writes a randomly named sibling
temporary file, flushes that file, and atomically renames it over the checkpoint.
It never deletes the last checkpoint before replacement. Unix also flushes the
parent directory. Use a local filesystem that supports atomic rename and file
locks; power-loss durability follows the operating system and filesystem.

If a process stops before the rename, the previous checkpoint is still the
committed state. An orphan `.tmp` is ignored by reads and listings. After
confirming there is no active writer, orphan temporary files can be removed.
If the checkpoint itself has been edited into invalid JSON, CodeSpace fails
closed and does not overwrite it. Recover the known-good file from a backup,
or create a new run from verified facts.

`list` returns compact summaries, ordered by ID, for at most 200 runs. The
response reports `truncated` when additional runs exist; named reads remain
available for those runs. Checkpoints are local working data, not a durable
audit log or version-control history.

## Trust and paths

Run IDs accept 1–64 ASCII letters, digits, underscores, and hyphens, beginning
with a letter or digit. Windows device names are rejected on every platform.
Storage directories, checkpoint files, and lock files reject symlinks and
Windows reparse points, including junctions. Active files must use safe
repository-relative forward-slash paths; existing linked path components are
rejected, and future paths are allowed only under the repository.

Likely credentials are redacted before persistence and again on read. This is
best-effort pattern matching, not a guarantee that every secret can be detected.
New Unix directories and files use owner-only permissions. Windows uses the
workspace's existing access-control settings.

The handoff prompt labels saved content as task data, with no authority to
override current instructions or grant permissions. Saved claims still need
fresh verification. An observation containing a command remains an observation.
The execution-state API never runs shell commands or calls an AI provider.

The local workspace and its owner are the security boundary. Validation rejects
linked paths present when checked; it does not sandbox a separate hostile
process that can concurrently replace directories owned by the same user.
