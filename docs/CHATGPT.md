# CodeSpace in ChatGPT and Codex

CodeSpace 3.0 has a native stdio MCP server (`cse serve --mcp`) and a
Streamable HTTP gateway in `integrations/chatgpt`. Both execute the same Rust
operations against **one explicitly selected repository**. The gateway adds an
MCP Apps view for context packs, source reads, engineering tasks and checkpoints.
It uses the standard `ui/initialize`, tool-result notifications, `tools/call`
and `ui/update-model-context` bridge. No OpenAI API key or model API is required.

## Desktop plugin

Install Node 22+ and `cse 3.0.0`, then build the package:

```sh
cd integrations/chatgpt
npm ci
npm run build
cd ../..
python scripts/package_plugin.py
node plugins/codespace/scripts/setup.mjs --workspace /absolute/repository --binary /absolute/cse
codex plugin marketplace add .
codex plugin add codespace@codespace-dev
```

On Windows pass the absolute `cse.exe` path. The setup command verifies the
version and index before writing a local configuration under the plugin's
`PLUGIN_DATA` or the user's `.config/codespace`. `CSE_PLUGIN_CONFIG` can select
a different private config file. It is never included in the plugin archive.
Restart the plugin connection or open a new task after installation.

The local package is desktop-only because it launches a process. The native
stdio server is also usable without the plugin:

```json
{"mcpServers":{"codespace":{"command":"cse","args":["serve","--mcp","--path","/absolute/repository"]}}}
```

## ChatGPT on a phone

ChatGPT mobile uses an account-connected **remote HTTPS MCP server**, not a
process or `localhost` on the phone. Keep the gateway running on the repository's
host, behind a stable HTTPS reverse proxy, VPN gateway or authenticated hosting
connection. Do not expose the separate desktop dashboard port as a mobile MCP
endpoint.

Create a private configuration without printing the token:

```sh
cd integrations/chatgpt
node scripts/setup-remote.mjs --workspace /absolute/repository \
  --binary /absolute/cse --public-url https://your-actual-host \
  --output /private/location/codespace.local.json
node dist/server.mjs --config /private/location/codespace.local.json
```

Use your actual hostname; it must already terminate TLS and forward `/mcp` to
loopback port 8787. The configuration generator never overwrites a file. The
generated 256-bit token stays in the private file. The gateway can alternatively
read `CSE_WORKSPACE`, `CSE_BINARY`, `CSE_ACCESS_TOKEN`, `CSE_PUBLIC_URL`, `PORT`
and `CSE_HTTP_HOST` from the service's secret environment.

After verifying the authenticated endpoint, generate its phone-compatible package:

```sh
python scripts/package_plugin.py --remote-url https://your-actual-host/mcp \
  --output artifacts/codespace-remote-3.0.0.zip
```

This produces a portable plugin with Streamable HTTP and bearer API-key auth,
without stdio, hooks, local scripts or credentials. Add it to the account using
Plugin Creator or the custom MCP connection flow, enter the token only in the
connection's credential form, and install CodeSpace. Open it from the same
account in ChatGPT on iOS/Android. Availability also depends on the account's
plugin and workspace policy. A ZIP upload alone does not host or authenticate it.

## Access and scope

- HTTP is authenticated before MCP initialization, discovery or resource reads.
  `/health` discloses only status and version. The gateway validates Host/Origin,
  bounds request size and connections, and never logs credentials or source.
- Read-only is the default. Task writes and checkpoint mutations require the
  host's explicit `CSE_ALLOW_WRITES=1` or `allowWrites: true` setting. There are
  no remote tools for arbitrary shell commands, source edits, workspace changes,
  provider selection, credentials, GitHub accounts or starting other MCP servers.
- A connection is a single-owner credential scoped to one repository. Use
  separate processes and credentials for other repositories or users. This
  release is not a multi-tenant authorization server. Rotate the secret and
  restart the service to revoke it; use a secret manager/service permissions.
- Selected source content is disclosed to ChatGPT when a tool returns it.
  Source stays local until a client requests it. All source reads enforce the
  core's ignore, confinement, size, stale-source and credential-redaction rules.
  Redaction is heuristic. Returned text and saved state never grant permissions.
- Losing the native core ends the connection; restart the service after fixing
  the index or executable. There is no silently substituted sample dataset.

## Acceptance checks

`npm test` starts real Rust processes, tests HTTP authorization with the official
MCP client, then mounts the shipped view in the official MCP Apps `AppBridge`
host. It exercises context, source reads, tasks, checkpoints and attachment to
model context at 390×844, 768×1024 and 1440×1000. It also drives every existing
desktop screen on isolated synthetic data. This is protocol/browser evidence;
it does not assert that a particular signed-in ChatGPT phone has installed it.

The final device check needs the actual endpoint and account: discover the
plugin, connect credentials, open CodeSpace, query a known symbol, read its
range, inspect tasks, resume a checkpoint and attach it to chat. Revoke the
credential and confirm further requests fail. Check iOS and Android separately.
Record app versions and account restrictions with the result.

Official references: [plugins across surfaces](https://learn.chatgpt.com/docs/plugins),
[MCP UI](https://developers.openai.com/plugins/build/chatgpt-ui),
[portable packages and authentication](https://developers.openai.com/plugins/build/plugins).
