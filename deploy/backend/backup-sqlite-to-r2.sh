#!/usr/bin/env bash
set -euo pipefail

for binary in sqlite3 zstd aws sha256sum; do
	if ! command -v "$binary" >/dev/null 2>&1; then
		echo "Missing required command: $binary" >&2
		exit 1
	fi
done

DB_PATH="${DB_PATH:-/var/lib/firewatch/indexer.db}"
BACKUP_TMP_DIR="${BACKUP_TMP_DIR:-/var/tmp/firewatch-backups}"
R2_ACCOUNT_ID="${R2_ACCOUNT_ID:?Set R2_ACCOUNT_ID}"
R2_BUCKET="${R2_BUCKET:?Set R2_BUCKET}"
R2_PREFIX="${R2_PREFIX:-sqlite-backups}"
R2_ENDPOINT_URL="${R2_ENDPOINT_URL:-https://${R2_ACCOUNT_ID}.r2.cloudflarestorage.com}"
AWS_DEFAULT_REGION="${AWS_DEFAULT_REGION:-auto}"
export AWS_DEFAULT_REGION
export AWS_EC2_METADATA_DISABLED=true

if [[ ! -f "$DB_PATH" ]]; then
	echo "SQLite database not found: $DB_PATH" >&2
	exit 1
fi

timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
partition="$(date -u +%Y/%m/%d)"
mkdir -p "$BACKUP_TMP_DIR"

snapshot="${BACKUP_TMP_DIR}/indexer-${timestamp}.db"
archive="${snapshot}.zst"
manifest="${archive}.sha256"

cleanup() {
	rm -f "$snapshot" "$archive" "$manifest"
}
trap cleanup EXIT

sqlite3 "$DB_PATH" ".backup '$snapshot'"
zstd -T0 -19 -q "$snapshot" -o "$archive"
sha256sum "$archive" > "$manifest"

object_key="${R2_PREFIX%/}/${partition}/indexer-${timestamp}.db.zst"
aws --endpoint-url "$R2_ENDPOINT_URL" s3 cp "$archive" "s3://${R2_BUCKET}/${object_key}" --only-show-errors
aws --endpoint-url "$R2_ENDPOINT_URL" s3 cp "$manifest" "s3://${R2_BUCKET}/${object_key}.sha256" --only-show-errors

echo "Uploaded s3://${R2_BUCKET}/${object_key}"
