#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
default_output_dir="$script_dir/../fixtures/yieldblox"
output_dir="$default_output_dir"
selection="checkpoints"

usage() {
  cat <<'EOF'
Usage:
  fetch-ledger-fixtures.sh [OUTPUT_DIR] [LEDGER ...]
  fetch-ledger-fixtures.sh [OUTPUT_DIR] --range START_LEDGER END_LEDGER

With no ledger arguments, fetches the seven YieldBlox ground-truth checkpoints.
A continuous range still needs a trusted active-ledger state snapshot immediately
before START_LEDGER to reconstruct pre-existing offers and contract/account state.
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

if [[ $# -gt 0 && "${1:-}" != "--range" ]]; then
  output_dir="$1"
  shift
fi

if [[ "${1:-}" == "--range" ]]; then
  if [[ $# -ne 3 ]]; then
    usage >&2
    exit 2
  fi

  range_start="$2"
  range_end="$3"
  if [[ ! "$range_start" =~ ^[0-9]+$ || ! "$range_end" =~ ^[0-9]+$ || "$range_start" -gt "$range_end" ]]; then
    echo "invalid ledger range: $range_start..$range_end" >&2
    exit 2
  fi

  ledger_sequences=()
  for ((ledger_sequence = range_start; ledger_sequence <= range_end; ledger_sequence++)); do
    ledger_sequences+=("$ledger_sequence")
  done
  selection="continuous-range"
elif [[ $# -gt 0 ]]; then
  ledger_sequences=("$@")
  selection="explicit-list"
else
  ledger_sequences=(
    61339940
    61340263
    61340315
    61340381
    61340384
    61340405
    61340408
  )
fi

rpc_url="${STELLAR_ARCHIVE_RPC_URL:-https://rpc.ankr.com/stellar_soroban}"
timeout_seconds="${RPC_FIXTURE_TIMEOUT_SECONDS:-60}"

for dependency in curl jq; do
  if ! command -v "$dependency" >/dev/null 2>&1; then
    echo "$dependency is required" >&2
    exit 1
  fi
done

for ledger_sequence in "${ledger_sequences[@]}"; do
  if [[ ! "$ledger_sequence" =~ ^[0-9]+$ ]]; then
    echo "invalid ledger sequence: $ledger_sequence" >&2
    exit 2
  fi
done

mkdir -p "$output_dir"
temporary_dir="$(mktemp -d "${TMPDIR:-/tmp}/ferno-ledger-fixtures.XXXXXX")"
trap 'rm -rf "$temporary_dir"' EXIT

for ledger_sequence in "${ledger_sequences[@]}"; do
  payload="$(
    jq -cn --argjson ledger "$ledger_sequence" '{
      jsonrpc: "2.0",
      id: ("fixture-" + ($ledger | tostring)),
      method: "getLedgers",
      params: {
        startLedger: $ledger,
        pagination: {limit: 1}
      }
    }'
  )"

  temporary_file="$temporary_dir/ledger-$ledger_sequence.json"
  destination_file="$output_dir/ledger-$ledger_sequence.json"

  curl \
    --fail \
    --silent \
    --show-error \
    --compressed \
    --max-time "$timeout_seconds" \
    --header 'Content-Type: application/json' \
    --data "$payload" \
    "$rpc_url" >"$temporary_file"

  if ! jq -e --argjson ledger "$ledger_sequence" '
    .error == null and
    .result.ledgers[0].sequence == $ledger and
    (.result.ledgers[0].hash | type == "string" and length > 0) and
    (.result.ledgers[0].headerXdr | type == "string" and length > 0) and
    (.result.ledgers[0].metadataXdr | type == "string" and length > 0)
  ' \
    "$temporary_file" >/dev/null; then
    jq -r '.error.message // "archive RPC returned the wrong or no ledger"' \
      "$temporary_file" >&2
    exit 1
  fi

  jq --sort-keys . "$temporary_file" >"$destination_file"
  ledger_hash="$(jq -r '.result.ledgers[0].hash' "$destination_file")"
  printf 'ledger %s %s\n' "$ledger_sequence" "$ledger_hash"
done

jq -n \
  --arg network "public" \
  --arg fetched_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg selection "$selection" \
  --argjson ledgers "$(printf '%s\n' "${ledger_sequences[@]}" | jq -R 'tonumber' | jq -s .)" \
  '{
    schema_version: 1,
    network: $network,
    fetched_at: $fetched_at,
    source: "archive-rpc",
    endpoint_recorded: false,
    selection: $selection,
    ledgers: $ledgers,
    starting_state_required: ($selection == "continuous-range"),
    purpose: "YieldBlox oracle-manipulation replay fixtures"
  }' >"$output_dir/manifest.json"

printf 'wrote %s fixtures to %s\n' "${#ledger_sequences[@]}" "$output_dir"
