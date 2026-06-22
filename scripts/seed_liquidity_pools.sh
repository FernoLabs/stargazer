#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/../backend"
exec cargo run -- seed-liquidity-pools "$@"
