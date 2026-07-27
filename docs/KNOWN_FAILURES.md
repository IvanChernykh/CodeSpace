# Known dashboard failure modes

This file records failures that must remain covered by automated tests.

## Multiple installed binaries

**Symptom:** source code is updated, but the dashboard still serves old routes or assets.  
**Cause:** more than one Cargo home or installation directory appears in `PATH`.  
**Prevention:** acceptance tests invoke the freshly built binary by absolute path. `cse doctor` must eventually report the resolved executable path and build identity.

## Public HTML with protected assets

**Symptom:** the page shell opens, then remains blank or loading forever. Browser requests for JavaScript or CSS return `401`.  
**Prevention:** `/assets/*` is public; repository data remains protected. The E2E test verifies both rules.

## EventSource authentication mismatch

**Symptom:** the UI displays `Connecting…` or an incorrect `Live` status.  
**Cause:** native `EventSource` cannot send an Authorization header; token transport and server validation diverge.  
**Prevention:** use an explicit, tested SSE authentication mechanism. Health polling must not masquerade as realtime connectivity.

## Silent dynamic port change

**Symptom:** the user opens the requested URL, but the server selected another port because an old process still owns it.  
**Prevention:** interactive startup may offer another port, but automated and installer modes must fail explicitly or report the final URL through a machine-readable channel.

## Generated asset drift

**Symptom:** TypeScript sources contain a fix while the embedded `dashboard/dist` files still contain old behavior.  
**Prevention:** CI rebuilds assets and fails when `git diff -- dashboard/dist` is non-empty.

## Feature-shaped placeholders

**Symptom:** a tab, button or endpoint exists, but the end-to-end workflow does not persist or return meaningful state.  
**Prevention:** each feature requires an acceptance workflow against the built binary, including empty, success and failure states.

## Dependency unavailable

**Symptom:** Ollama, GitHub or an MCP server is missing and the UI spins indefinitely or reports generic failure.  
**Prevention:** integrations expose typed states: `unconfigured`, `starting`, `ready`, `degraded`, `failed`; each state includes a user action.
