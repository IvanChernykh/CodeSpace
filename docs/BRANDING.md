# Branding and command-collision decision

The requested product name is **CodeSpace** and the primary binary is **`cse`**. The implementation now uses those identifiers consistently.

## Critical release risk

- GitHub operates **GitHub Codespaces**, creating strong semantic and search overlap.
- GitHub CLI exposes `gh cs` as an official alias for `gh codespace`. The standalone `cse` executable avoids that command collision, although the product-name overlap remains.
- Crate, package-manager, domain, social-handle, and trademark availability remain release-gating checks.

## Current technical identifiers

| Surface | Identifier |
|---|---|
| Product | `CodeSpace` |
| Shell binary | `cse` |
| Rust package | `codespace-cse` |
| Rust library crate | `codespace` |
| MCP server key | `codespace` |
| MCP tools | `cse_search`, `cse_context`, `cse_impact`, `cse_history`, `cse_read` |
| Local state directory | `.codespace/` |
| Ignore file | `.codespaceignore` |

## Acceptance gate before public publication

1. Obtain a trademark/domain review in target jurisdictions.
2. Verify package availability immediately before publication; registry allocation is first-come, first-served.
3. Test `cse` installation against common shells and existing developer toolchains.
4. Retain `cse` as the only supported short command; do not publish a `cs` alias.
5. Record the final naming decision in the release checklist.
