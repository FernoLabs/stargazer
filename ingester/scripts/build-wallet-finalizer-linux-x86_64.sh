#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
manifest="$repo_root/ingester/reader-rs/Cargo.toml"

if ! command -v cargo-zigbuild >/dev/null 2>&1; then
  echo "cargo-zigbuild is required (install with: cargo install cargo-zigbuild)" >&2
  exit 1
fi

env \
  -u CPATH \
  -u C_INCLUDE_PATH \
  -u CPLUS_INCLUDE_PATH \
  -u LIBRARY_PATH \
  cargo zigbuild \
    --release \
    --locked \
    --manifest-path "$manifest" \
    --bin stargazer-wallet-finalizer \
    --target x86_64-unknown-linux-musl

echo "$repo_root/ingester/reader-rs/target/x86_64-unknown-linux-musl/release/stargazer-wallet-finalizer"
