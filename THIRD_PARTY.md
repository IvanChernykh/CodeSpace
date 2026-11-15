# Third-party dependencies and provenance

CodeSpace is an independently implemented repository tool. Product research does
not grant permission to copy another project's source; its upstream license
always applies to that source.

The Rust core uses `getrandom`, `serde_json` and `fs2`, with transitive dependencies
pinned in `Cargo.lock`. The optional Node gateway uses the official MIT-licensed
MCP SDK and MCP Apps SDK plus Ajv. Its browser/server bundles are built with
esbuild; Playwright is a development/test dependency. Exact versions and integrity
digests are in `integrations/chatgpt/package-lock.json`. The distributed local
plugin includes bundled dependencies' license texts in `server/licenses.json`.

Before importing other source, record its upstream repository and immutable
commit, file-level license and copyright, Apache-2.0 compatibility, required
attribution, maintenance plan and SBOM entry. The project's Apache-2.0 license
does not relicense third-party code.
