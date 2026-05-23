# Dokploy Indexer Deployment

Use `docker-compose.dokploy.yml` at the repository root for the Dokploy Compose app.

## Dokploy Settings

- Provider: Git repository
- Compose file: `docker-compose.dokploy.yml`
- Service domain target: `firewatch-indexer`, port `8080`
- Volume: keep `firewatch-indexer-data` as a Docker named volume
- Environment: paste values from `deploy/dokploy/.env.example` into Dokploy's Environment editor

Dokploy writes the Environment editor values to `.env`. The compose file uses `env_file: .env` so the backend container receives the indexer settings.

## Backups

Use Dokploy Volume Backups for the named volume if you want platform-managed backups.

For SQLite-consistent R2 backups, create a Dokploy Schedule that runs:

```bash
docker compose --profile backup run --rm firewatch-r2-backup
```

The backup service uses SQLite's `.backup` command, compresses the snapshot with `zstd`, and uploads the archive plus a SHA-256 manifest to R2.

## Backfills

Run one-off commands against the same data volume from Dokploy's terminal or an SSH shell:

```bash
docker compose run --rm firewatch-indexer backfill-blocks 400000000 400001000 --concurrency 4 --retries 2
```

Avoid running heavy backfills at the same time as a live indexer on a very small VM unless you have tested CPU and I/O headroom.
