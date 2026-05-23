#!/usr/bin/env bash
set -euo pipefail

BLOB_STORE_DIR="${BLOB_STORE_DIR:-/blobs}"
R2_ACCOUNT_ID="${R2_ACCOUNT_ID:?Set R2_ACCOUNT_ID}"
R2_BLOB_BUCKET="${R2_BLOB_BUCKET:-${R2_BUCKET:-}}"
R2_BLOB_BUCKET="${R2_BLOB_BUCKET:?Set R2_BLOB_BUCKET}"
R2_BLOB_PREFIX="${R2_BLOB_PREFIX:-blobs}"
R2_ENDPOINT_URL="${R2_ENDPOINT_URL:-https://${R2_ACCOUNT_ID}.r2.cloudflarestorage.com}"

if [[ ! -d "$BLOB_STORE_DIR" ]]; then
	echo "Blob store directory does not exist: $BLOB_STORE_DIR" >&2
	exit 1
fi

destination="s3://${R2_BLOB_BUCKET}"
if [[ -n "$R2_BLOB_PREFIX" ]]; then
	destination="${destination}/${R2_BLOB_PREFIX%/}"
fi

aws --endpoint-url "$R2_ENDPOINT_URL" s3 sync "${BLOB_STORE_DIR%/}/" "${destination}/" --size-only --only-show-errors

echo "Synced ${BLOB_STORE_DIR} to ${destination}/"
