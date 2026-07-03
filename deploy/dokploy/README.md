# Dokploy Indexer Deployment

Use `docker-compose.dokploy.yml` at the repository root for the Dokploy Compose app.

## Dokploy Settings

- Provider: Git repository
- Compose file: `docker-compose.dokploy.yml`
- Service domain target: none when using Cloudflare Tunnel
- Volumes: keep `firewatch-indexer-data` and `firewatch-blob-data` as Docker named volumes
- Environment: paste values from `deploy/dokploy/.env.example` into Dokploy's Environment editor
- Tunnel: set `COMPOSE_PROFILES=tunnel` and fill `CLOUDFLARE_TUNNEL_TOKEN`

Dokploy writes the Environment editor values to `.env`. The compose file uses `env_file: .env` so the backend container receives the indexer settings.

The tunnel profile runs `firewatch-cloudflared` and forwards `firewatch-origin.ferno.ag` to `http://firewatch-indexer:8080`. Keep the indexer off a public Dokploy domain for the first deployment.

## Blob Store

The backend stores large assembled program bytes, IDLs, disassemblies, and generated textual program diffs in `BLOB_STORE_DIR=/blobs`. If `BLOB_STORE_S3_BUCKET` is set, it also writes those generated blobs to S3/R2 immediately, which keeps split-view disassembly artifacts available outside SQLite and off the VM volume.

Program bytecode and diffs are stored under one R2/object-store prefix per program. Full bytecode snapshots use `programs/{program}/versions/{slot}-{version_id}/program-{sha}.bin`; textual diffs use `programs/{program}/diffs/{slot}-{version_id}/...patch`. Raw loader write payloads are compacted out of SQLite after a complete full-version bytecode snapshot has been assembled, while the write metadata, offsets, lengths, hashes, and ordering remain in SQLite.

For Cloudflare R2 live blob storage, set `R2_BLOB_BUCKET=firewatch-blobs`, `R2_BLOB_PREFIX=blobs`, `AWS_DEFAULT_REGION=auto`, `R2_ACCOUNT_ID`, and the R2 `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY` pair. `BLOB_STORE_S3_BUCKET`, `BLOB_STORE_S3_PREFIX`, `BLOB_STORE_S3_ENDPOINT_URL`, and `BLOB_STORE_S3_REGION` can override those backup-oriented vars when the live blob store needs a separate bucket or endpoint.

For an existing database, pause the live indexer, then run this once after the blob volume or S3 store is configured:

```bash
docker compose run --rm firewatch-indexer migrate-blobs --vacuum
```

The Cloudflare R2 bucket for off-machine blob copies is `firewatch-blobs`.

## Backups

Use Dokploy Volume Backups for both named volumes if you want platform-managed backups.

For SQLite-consistent R2 backups, create a Dokploy Schedule that runs:

```bash
docker compose --profile backup run --rm firewatch-r2-backup
```

The backup service uses SQLite's `.backup` command, compresses the snapshot with `zstd`, and uploads the archive plus a SHA-256 manifest to R2.

After blobs are enabled, also schedule:

```bash
docker compose --profile backup run --rm firewatch-r2-blob-sync
```

That syncs `/blobs` to the `firewatch-blobs` R2 bucket.

## Backfills

Run one-off commands against the same data volume from Dokploy's terminal or an SSH shell:

```bash
docker compose run --rm firewatch-indexer backfill-blocks 400000000 400001000 --concurrency 4 --retries 2
```

For a program upgrade where FireWatch has the new event but no prior version to compare against, replay recent upgrade windows for that program:

```bash
docker compose run --rm firewatch-indexer backfill-program-history JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4 --limit 2 --lookback-slots 2048 --concurrency 4 --retries 2
```

Refresh protocol-linked Squads multisigs from current program upgrade authorities:

```bash
docker compose run --rm firewatch-indexer sync-program-authorities
```

Use `--discover-all-squads` only with an RPC provider that can handle a large Squads `getProgramAccounts` response.

Avoid running heavy backfills at the same time as a live indexer on a very small VM unless you have tested CPU and I/O headroom.
