#!/usr/bin/env bash

set -euo pipefail

reader="${STARGAZER_READER_BIN:-/srv/stargazer/bin/stargazer-ledger-reader}"
archive="${STARGAZER_ARCHIVE_ROOT:-/srv/stargazer/raw/galexie/pubnet}"
database="${STARGAZER_INDEX_DB:-/srv/stargazer/index/stargazer-one-year-compact.sqlite}"
protocol_roots="${STARGAZER_PROTOCOL_ROOTS:-/srv/stargazer/config/stellar-mainnet-defi-roots.json}"
retention_days="${STELLAR_RETENTION_DAYS:-365}"
decode_workers="${STARGAZER_DECODE_WORKERS:-2}"
decode_ahead="${STARGAZER_DECODE_AHEAD:-4}"
batch_ledgers="${STARGAZER_BATCH_LEDGERS:-2000}"

for value in "$retention_days" "$decode_workers" "$decode_ahead" "$batch_ledgers"; do
  if [[ ! "$value" =~ ^[1-9][0-9]*$ ]]; then
    echo "retention and concurrency values must be positive integers" >&2
    exit 2
  fi
done
if [[ ! -x "$reader" ]]; then
  echo "reader is not executable: $reader" >&2
  exit 1
fi
if [[ ! -d "$archive" ]]; then
  echo "archive root is not a directory: $archive" >&2
  exit 1
fi
if [[ ! -f "$protocol_roots" ]]; then
  echo "protocol root registry is missing: $protocol_roots" >&2
  exit 1
fi

mkdir -p "$(dirname "$database")"
exec 9>"$database.indexer.lock"
if ! flock -n 9; then
  echo "another compact indexer holds $database.indexer.lock" >&2
  exit 1
fi

partition_file="$(mktemp "${TMPDIR:-/tmp}/stargazer-compact-partitions.XXXXXX")"
cleanup() {
  rm -f "$partition_file"
}
trap cleanup EXIT INT TERM

newest_partition="$({
  find "$archive" -mindepth 1 -maxdepth 1 -type d -printf '%f\n'
} | LC_ALL=C sort | head -n 1)"
if [[ -z "$newest_partition" ]]; then
  echo "archive contains no partition directories" >&2
  exit 1
fi
newest_start="${newest_partition#*--}"
newest_start="${newest_start%%-*}"
cutoff_ledger=$((newest_start - retention_days * 17280))
if [[ "$cutoff_ledger" -lt 0 ]]; then
  cutoff_ledger=0
fi

while IFS= read -r path; do
  partition="${path##*/}"
  start="${partition#*--}"
  start="${start%%-*}"
  end="${partition##*-}"
  if [[ "$partition" == "$newest_partition" || "$end" -lt "$cutoff_ledger" ]]; then
    continue
  fi
  printf '%s\t%s\n' "$start" "$path"
done < <(find "$archive" -mindepth 1 -maxdepth 1 -type d) \
  | LC_ALL=C sort -n -k1,1 > "$partition_file"

partition_count="$(wc -l < "$partition_file" | tr -d ' ')"
if [[ "$partition_count" -eq 0 ]]; then
  echo "no closed partitions found in the last $retention_days days" >&2
  exit 1
fi

echo "$(date -Iseconds) compact-indexing $partition_count closed partitions from ledger $cutoff_ledger; live partition $newest_partition excluded"
while IFS=$'\t' read -r start partition_path; do
  echo "$(date -Iseconds) compact-indexing partition ${partition_path##*/}"
  "$reader" compact-index \
    --root "$partition_path" \
    --db "$database" \
    --protocol-roots "$protocol_roots" \
    --decode-workers "$decode_workers" \
    --decode-ahead "$decode_ahead" \
    --batch-ledgers "$batch_ledgers"
done < "$partition_file"

echo "$(date -Iseconds) one-year compact index complete: $database"
