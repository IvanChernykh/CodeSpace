# Recovery status

- Baseline commit: `b500489fcbcbb6cc9f635cdfe6c2eb4a031370b6`
- Recovery branch: `recovery/dashboard-stability`
- Current phase: R0 — executable stability gates

## Added

- Cross-platform dashboard E2E workflow.
- Fresh-binary smoke harness.
- Recovery architecture and release gates.
- Acceptance checklist and known failure catalogue.

## Next implementation slice

1. Make the new E2E workflow green against the current server contract.
2. Correct structured integration states and realtime status semantics.
3. Consolidate tasks, AI and GitHub routes through the application registry.
4. Reconstruct navigation and primary workflows after contract stabilization.

No recovery changes are merged directly to `main` until CI evidence is available.
