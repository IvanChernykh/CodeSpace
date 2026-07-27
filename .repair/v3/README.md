# Verified CodeSpace v3 repair payload

This temporary directory is consumed by `.github/workflows/dashboard-v3-repair.yml`.
The workflow reconstructs the payload, verifies SHA-256, applies the repair, runs frontend and Rust quality gates, then removes this directory before committing generated changes.
