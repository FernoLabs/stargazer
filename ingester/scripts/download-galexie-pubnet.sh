#!/usr/bin/env bash

set -euo pipefail

s5cmd_bin="${S5CMD_BIN:-s5cmd}"
destination="${STARGAZER_ARCHIVE_ROOT:-/srv/stargazer/archive/galexie/pubnet}"
state_dir="${STARGAZER_DOWNLOAD_STATE_DIR:-${destination}.state}"
s3_source="${STELLAR_GALEXIE_S3:-s3://aws-public-blockchain/v1.1/stellar/ledgers/pubnet}"
workers="${S5CMD_WORKERS:-64}"
retry_count="${S5CMD_RETRY_COUNT:-20}"
partition_limit="${STARGAZER_PARTITION_LIMIT:-0}"
retention_days="${STELLAR_RETENTION_DAYS:-365}"

usage() {
  cat <<'EOF'
Download the public Stellar Galexie pubnet archive one 64,000-ledger
partition at a time. Existing files are reused by size and no files are deleted.

Environment:
  S5CMD_BIN                       s5cmd executable
  STARGAZER_ARCHIVE_ROOT          archive destination
  STARGAZER_DOWNLOAD_STATE_DIR    completion-marker directory
  STELLAR_RETENTION_DAYS          keep/download this many days from the live tip (default 365)
  S5CMD_WORKERS                   concurrent small-file transfers (default 64)
  S5CMD_RETRY_COUNT               retries per request (default 20)
  STARGAZER_PARTITION_LIMIT       stop after N attempted partitions; 0 means all
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi
if [[ $# -ne 0 ]]; then
  usage >&2
  exit 2
fi
if [[ ! "$workers" =~ ^[1-9][0-9]*$ ]]; then
  echo "S5CMD_WORKERS must be a positive integer" >&2
  exit 2
fi
if [[ ! "$retry_count" =~ ^[0-9]+$ ]]; then
  echo "S5CMD_RETRY_COUNT must be a non-negative integer" >&2
  exit 2
fi
if [[ ! "$partition_limit" =~ ^[0-9]+$ ]]; then
  echo "STARGAZER_PARTITION_LIMIT must be a non-negative integer" >&2
  exit 2
fi
if [[ ! "$retention_days" =~ ^[1-9][0-9]*$ ]]; then
  echo "STELLAR_RETENTION_DAYS must be a positive integer" >&2
  exit 2
fi

for dependency in "$s5cmd_bin" mktemp flock; do
  if ! command -v "$dependency" >/dev/null 2>&1; then
    echo "$dependency is required" >&2
    exit 1
  fi
done

mkdir -p "$destination" "$state_dir"
exec 9>"$state_dir/downloader.lock"
if ! flock -n 9; then
  echo "another Galexie downloader holds $state_dir/downloader.lock" >&2
  exit 1
fi

partition_file="$(mktemp "${TMPDIR:-/tmp}/stargazer-partitions.XXXXXX")"
cleanup() {
  rm -f "$partition_file"
}
trap cleanup EXIT INT TERM

"$s5cmd_bin" --no-sign-request ls "$s3_source/" \
  | awk '$1 == "DIR" { sub(/\/$/, "", $2); print $2 }' \
  | LC_ALL=C sort \
  > "$partition_file"

partition_count="$(wc -l < "$partition_file" | tr -d ' ')"
if [[ "$partition_count" -eq 0 ]]; then
  echo "the public archive returned no partitions" >&2
  exit 1
fi
if grep -Ev '^[0-9A-F]{8}--[0-9]+-[0-9]+$' "$partition_file" >/dev/null; then
  echo "the public archive returned an unexpected partition name" >&2
  exit 1
fi

IFS= read -r newest_partition < "$partition_file"
newest_start="${newest_partition#*--}"
newest_start="${newest_start%%-*}"
ledgers_per_day=17280
retention_ledgers=$((retention_days * ledgers_per_day))
cutoff_ledger=$((newest_start - retention_ledgers))
if [[ "$cutoff_ledger" -lt 0 ]]; then
  cutoff_ledger=0
fi
echo "found $partition_count partitions; live partition is $newest_partition; downloading partitions through ledger $cutoff_ledger (last $retention_days days)"

attempted=0
while IFS= read -r partition; do
  if [[ "$partition_limit" -gt 0 && "$attempted" -ge "$partition_limit" ]]; then
    break
  fi

  partition_end="${partition##*-}"
  if [[ "$partition_end" -lt "$cutoff_ledger" ]]; then
    continue
  fi

  marker="$state_dir/$partition.complete"
  if [[ "$partition" != "$newest_partition" && -f "$marker" ]]; then
    continue
  fi

  attempted=$((attempted + 1))
  mkdir -p "$destination/$partition"
  echo "$(date -Iseconds) syncing partition $partition ($attempted attempted this run)"
  "$s5cmd_bin" \
    --no-sign-request \
    --log error \
    --stat \
    --numworkers "$workers" \
    --retry-count "$retry_count" \
    sync --size-only \
    "$s3_source/$partition/*" \
    "$destination/$partition/"

  if [[ "$partition" != "$newest_partition" ]]; then
    touch "$marker"
  fi
done < "$partition_file"

echo "$(date -Iseconds) partition pass complete"
