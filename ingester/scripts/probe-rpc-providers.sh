#!/usr/bin/env bash

set -euo pipefail

historical_ledger="${1:-50859939}"
timeout_seconds="${RPC_PROBE_TIMEOUT_SECONDS:-45}"

if [[ ! "$historical_ledger" =~ ^[0-9]+$ ]]; then
  echo "usage: $0 [historical-ledger-sequence]" >&2
  exit 2
fi

for dependency in curl jq; do
  if ! command -v "$dependency" >/dev/null 2>&1; then
    echo "$dependency is required" >&2
    exit 1
  fi
done

providers=(
  "gateway|live+archive|https://rpc.mainnet.stellar.gateway.fm"
  "ankr|live+archive|https://rpc.ankr.com/stellar_soroban"
  "onfinality|live+archive|https://stellar.api.onfinality.io/public"
  "lightsail-live|live|https://rpc.lightsail.network/"
  "lightsail-archive|archive|https://archive-rpc.lightsail.network/"
  "sorobanrpc.com|live|https://mainnet.sorobanrpc.com"
)

if [[ -n "${STELLAR_RPC_URL:-}" ]]; then
  providers+=("configured-live|live|${STELLAR_RPC_URL}")
fi

if [[ -n "${STELLAR_ARCHIVE_RPC_URL:-}" ]]; then
  providers+=("configured-archive|archive|${STELLAR_ARCHIVE_RPC_URL}")
fi

health_payload='{"jsonrpc":"2.0","id":"health","method":"getHealth"}'
network_payload='{"jsonrpc":"2.0","id":"network","method":"getNetwork"}'
historical_payload="$(
  jq -cn --argjson ledger "$historical_ledger" '{
    jsonrpc: "2.0",
    id: "historical-ledger",
    method: "getLedgers",
    params: {
      startLedger: $ledger,
      pagination: {limit: 1}
    }
  }'
)"

rpc_call() {
  local url="$1"
  local payload="$2"

  curl \
    --silent \
    --show-error \
    --compressed \
    --max-time "$timeout_seconds" \
    --header 'Content-Type: application/json' \
    --data "$payload" \
    "$url"
}

short_error() {
  local response="$1"
  local message

  message="$(jq -r '.error.message // .message // empty' <<<"$response" 2>/dev/null || true)"
  if [[ -z "$message" ]]; then
    message="unavailable"
  fi
  printf '%.42s' "$message"
}

printf 'Historical capability probe: getLedgers(%s)\n\n' "$historical_ledger"
printf '%-19s %-13s %-8s %-10s %-11s %-11s %s\n' \
  "PROVIDER" "ROLE" "NETWORK" "HEALTH" "OLDEST" "LATEST" "HISTORICAL"
printf '%-19s %-13s %-8s %-10s %-11s %-11s %s\n' \
  "-------------------" "-------------" "--------" "----------" "-----------" "-----------" "----------"

for provider in "${providers[@]}"; do
  IFS='|' read -r name role url <<<"$provider"

  health_response=""
  network_response=""
  historical_response=""

  health_response="$(rpc_call "$url" "$health_payload" 2>&1 || true)"
  network_response="$(rpc_call "$url" "$network_payload" 2>&1 || true)"
  historical_response="$(rpc_call "$url" "$historical_payload" 2>&1 || true)"

  health="$(jq -r '.result.status // .status // empty' <<<"$health_response" 2>/dev/null || true)"
  oldest="$(jq -r '.result.oldestLedger // "-"' <<<"$health_response" 2>/dev/null || printf '-')"
  latest="$(jq -r '.result.latestLedger // "-"' <<<"$health_response" 2>/dev/null || printf '-')"
  passphrase="$(jq -r '.result.passphrase // empty' <<<"$network_response" 2>/dev/null || true)"
  returned_ledger="$(jq -r '.result.ledgers[0].sequence // empty' <<<"$historical_response" 2>/dev/null || true)"

  if [[ "$passphrase" == "Public Global Stellar Network ; September 2015" ]]; then
    network="public"
  elif [[ -n "$passphrase" ]]; then
    network="other"
  else
    network="unknown"
  fi

  if [[ -z "$health" ]]; then
    health="$(short_error "$health_response")"
  fi

  if [[ "$returned_ledger" == "$historical_ledger" ]]; then
    ledger_hash="$(jq -r '.result.ledgers[0].hash // empty' <<<"$historical_response")"
    historical="yes:${ledger_hash:0:12}"
  else
    historical="no:$(short_error "$historical_response")"
  fi

  printf '%-19s %-13s %-8s %-10.10s %-11s %-11s %s\n' \
    "$name" "$role" "$network" "$health" "$oldest" "$latest" "$historical"
done

printf '\nArchive RPC extends getLedgers only. Public endpoints are rate-limited and have no POC SLA.\n'
