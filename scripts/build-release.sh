#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
VERSION="$(awk -F '"' '/^version = / {print $2; exit}' Cargo.toml)"
TARGET="${TARGET:-$(rustc -vV | awk '/host:/ {print $2}')}"
NAME="codespace-cse-${VERSION}-${TARGET}"
rm -rf dist
mkdir -p "dist/${NAME}"
cargo build --release --locked --target "$TARGET"
BINARY="target/${TARGET}/release/cse"
if [[ "$TARGET" == *windows* ]]; then BINARY="${BINARY}.exe"; fi
cp "$BINARY" "dist/${NAME}/"
cp README.md LICENSE NOTICE SECURITY.md CHANGELOG.md "dist/${NAME}/"
if [[ "$TARGET" == *windows* ]]; then
  (cd dist && zip -qr "${NAME}.zip" "$NAME")
  sha256sum "dist/${NAME}.zip" > "dist/${NAME}.zip.sha256"
else
  tar -C dist -czf "dist/${NAME}.tar.gz" "$NAME"
  sha256sum "dist/${NAME}.tar.gz" > "dist/${NAME}.tar.gz.sha256"
fi
printf 'Built release under %s/dist\n' "$ROOT"
