## Summary

Describe the user-facing workflow changed by this pull request.

## Surfaces

- [ ] CLI
- [ ] Dashboard / REST API
- [ ] MCP
- [ ] Persistence / migration
- [ ] Packaging / installation

## Verification

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test --locked`
- [ ] Dashboard TypeScript build
- [ ] `scripts/dashboard_smoke.py`
- [ ] Manual workflow verification

## UI evidence

Attach before/after screenshots or a short recording for visible changes. Include loading, empty, degraded and error states when relevant.

## Contract parity

Explain whether CLI, API/UI and MCP use the same application action. Document any intentional surface difference.

## Risks and rollback

List persistence, security, compatibility and installation risks. State the rollback path.
