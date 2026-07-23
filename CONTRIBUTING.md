# Contributing

## Requirements

- Rust version pinned in `rust-toolchain.toml`.
- No `unsafe` code.
- New behavior requires tests and acceptance evidence.
- Do not copy source from inspiration projects. Reimplement from documented behavior and record provenance.

## Local checks

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
python3 scripts/self_test.py
```

## Pull requests

Include problem statement, design choice, security impact, compatibility impact, tests, benchmark evidence when performance is claimed, and license/provenance notes for every new dependency.
