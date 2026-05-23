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

The backend stores large assembled program bytes, IDLs, and disassemblies in `BLOB_STORE_DIR=/blobs`. Raw write chunks remain in SQLite for now to avoid hundreds of thousands of tiny filesystem objects. For an existing database, pause the live indexer, then run this once after the blob volume is mounted:

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

Avoid running heavy backfills at the same time as a live indexer on a very small VM unless you have tested CPU and I/O headroom.
