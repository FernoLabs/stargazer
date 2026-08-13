#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
manifest="$repo_root/ingester/reader-rs/Cargo.toml"

if ! command -v cargo-zigbuild >/dev/null 2>&1; then
  echo "cargo-zigbuild is required (install with: cargo install cargo-zigbuild)" >&2
  exit 1
fi

# Homebrew/CommandLineTools can export macOS SDK include paths through CPATH.
# Those headers must not leak into Zig's Linux sysroot.
env \
  -u CPATH \
  -u C_INCLUDE_PATH \
  -u CPLUS_INCLUDE_PATH \
  -u LIBRARY_PATH \
  cargo zigbuild \
    --release \
    --locked \
    --manifest-path "$manifest" \
    --target x86_64-unknown-linux-musl

echo "$repo_root/ingester/reader-rs/target/x86_64-unknown-linux-musl/release/stargazer-ledger-reader"
