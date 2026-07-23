#!/usr/bin/env sh
set -eu
RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-1.97.1}"
if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required; install Rust with rustup first" >&2
  exit 1
fi
cargo "+${RUST_TOOLCHAIN}" install --path "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)" --locked
