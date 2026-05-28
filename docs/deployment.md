# FireWatch Deployment Runbook

This setup keeps the expensive, stateful work on one low-cost origin and pushes global reads through Cloudflare.

## Target Topology

```text
users
  -> Cloudflare Pages / Worker frontend
  -> /api/backend/* same-origin proxy with edge cache
  -> Cloudflare Tunnel firewatch-origin.ferno.ag
  -> Dokploy on a low-cost VPS running firewatch-indexer privately
  -> SQLite on a Docker named volume backed by local NVMe
  -> R2 for database backups and, later, large immutable blobs
```

## Cloudflare Pages Frontend

The frontend uses `@sveltejs/adapter-cloudflare`, `frontend/wrangler.toml`, and a GitHub Actions Direct Upload workflow.

Current project:

- Production branch: `main`
- Project name: `firewatch-frontend`
- Pages URL: `https://firewatch-frontend-5rr.pages.dev`
- Custom domain: `https://watch.ferno.ag`
- Tunnel name: `firewatch-origin`
- Tunnel ID: `24bab5c9-01d6-4448-a35e-94280db763db`
- Tunnel hostname: `https://firewatch-origin.ferno.ag`

GitHub Actions runs `npm ci`, `npm run check`, `npm run build`, and then deploys `.svelte-kit/cloudflare` to Cloudflare Pages. Pushes to `main` deploy production. Other branches and pull requests deploy preview branches.

Required GitHub Actions secrets:

| Name | Value |
| --- | --- |
| `CLOUDFLARE_ACCOUNT_ID` | Cloudflare account id for the Pages project |
| `CLOUDFLARE_API_TOKEN` | Scoped token with Pages write access |

The repo already has `CLOUDFLARE_ACCOUNT_ID` configured. Add `CLOUDFLARE_API_TOKEN` before relying on automatic deploys. The workflow still runs checks when the token is missing and skips only the deploy step.

### Pages Environment

Keep non-secret public defaults in `frontend/wrangler.toml`:

```toml
PUBLIC_BACKEND_BASE_URL = "/api/backend"
PUBLIC_SOLANA_EXPLORER_CLUSTER = "mainnet-beta"
```

Set these Cloudflare Pages variables/secrets for both Production and Preview environments:

| Name | Value |
| --- | --- |
| `BACKEND_ORIGIN_URL` | `https://firewatch-origin.ferno.ag` |
| `BACKEND_ORIGIN_TOKEN` | Same value as backend `ORIGIN_API_TOKEN` |
| `ADMIN_USERNAME` | Usually `admin` |
| `ADMIN_PASSWORD` | Strong private password for `/admin` |
| `NODE_VERSION` | Optional when `frontend/.node-version` is present, but `22.16.0` is safe |

Use a staging Dokploy origin for Preview when possible. If previews point at production, keep preview access restricted because the admin page can read and mutate production alert rules after authentication.

### Origin Protection

Set the same random token on the Dokploy backend:

```bash
ORIGIN_API_TOKEN=<same-value-as-BACKEND_ORIGIN_TOKEN>
```

When `ORIGIN_API_TOKEN` is set, the backend rejects direct `/api/*` requests unless they include `x-firewatch-origin-token`. `/healthz` stays public for Dokploy, Caddy, and load balancer health checks.

The browser calls same-origin paths such as `/api/backend/api/events`. The Pages function injects the origin token, applies small edge-cache windows to public reads, and blocks protected admin API paths unless `/admin` Basic Auth has already succeeded.

### DNS Records

Create these Cloudflare DNS records in the `ferno.ag` zone:

| Type | Name | Target | Proxy |
| --- | --- | --- | --- |
| `CNAME` | `watch` | `firewatch-frontend-5rr.pages.dev` | Proxied |
| `CNAME` | `firewatch-origin` | `24bab5c9-01d6-4448-a35e-94280db763db.cfargotunnel.com` | Proxied |

The origin hostname is only a Cloudflare Tunnel route. It does not require opening inbound ports on the backend VM.

### Manual Pages Deploy

Use this for a one-off smoke test or when bootstrapping the project locally:

```bash
cd frontend
npm ci
npm run check
npm run build
npx wrangler pages deploy .svelte-kit/cloudflare --project-name firewatch-frontend
```

## Dokploy Origin

Provision one EU VM first, install Dokploy there, and deploy the indexer as a Docker Compose app. In Dokploy:

- Provider: Git repository
- Compose file: `docker-compose.dokploy.yml`
- Domain target: none when using Cloudflare Tunnel
- Data persistence: keep the `firewatch-indexer-data` and `firewatch-blob-data` Docker named volumes
- Environment: paste values from `deploy/dokploy/.env.example` into the Environment editor
- Origin API token: set `ORIGIN_API_TOKEN` to match the Cloudflare Pages `BACKEND_ORIGIN_TOKEN`
- Tunnel profile: set `COMPOSE_PROFILES=tunnel`
- Tunnel token: set `CLOUDFLARE_TUNNEL_TOKEN` from Cloudflare Zero Trust or the private bootstrap env file

The compose file uses `env_file: .env` because Dokploy writes Environment editor values to a `.env` file next to the compose deployment.

The `tunnel` profile runs `firewatch-cloudflared` next to the backend. Cloudflare Tunnel is configured to forward:

```text
firewatch-origin.ferno.ag -> http://firewatch-indexer:8080
```

Keep the Dokploy app off the public internet unless you intentionally add a separate protected origin hostname later.

The backend container is built from `deploy/dokploy/backend.Dockerfile` and stores SQLite at:

```text
/data/indexer.db
```

The compose volume maps that to the Docker named volume:

```text
firewatch-indexer-data
firewatch-blob-data
```

This is the right default for Dokploy because named volumes are portable and can be used with Dokploy Volume Backups.

### Dokploy R2 Backups

For platform-managed backups, enable Dokploy Volume Backups on both named volumes.

For SQLite-consistent R2 backups, use the optional backup profile. Fill the R2 variables in Dokploy's Environment editor, then create a Dokploy Schedule that runs:

```bash
docker compose --profile backup run --rm firewatch-r2-backup
```

The backup service runs SQLite's `.backup`, compresses the snapshot with `zstd`, uploads it to R2, and uploads a SHA-256 manifest next to it.

After blobs are enabled, also schedule:

```bash
docker compose --profile backup run --rm firewatch-r2-blob-sync
```

### Dokploy Backfills

Run one-off backfills against the same volume:

```bash
docker compose run --rm firewatch-indexer backfill-blocks 400000000 400001000 --concurrency 4 --retries 2
```

To recover program-version context after a newly observed upgrade, backfill the recent upgrade history for that program. This resolves the program-data account, finds recent upgrade signatures, and replays a slot window around each one so buffer writes are available for diffing:

```bash
docker compose run --rm firewatch-indexer backfill-program-history JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4 --limit 2 --lookback-slots 2048 --concurrency 4 --retries 2
```

For a small VM, pause the live indexer or run the backfill during a quiet window if CPU or disk I/O gets tight.

## Manual Systemd Origin

Use this if you decide not to use Dokploy for an indexer host.

Provision one EU VM first. Use Debian or Ubuntu LTS, local NVMe storage, and a dedicated service user.

Install runtime tools:

```bash
sudo apt-get update
sudo apt-get install -y ca-certificates curl fonts-dejavu-core sqlite3 zstd awscli caddy
sudo useradd --system --home /var/lib/firewatch --create-home --shell /usr/sbin/nologin firewatch
sudo install -d -o firewatch -g firewatch /opt/firewatch/bin /etc/firewatch /var/lib/firewatch
```

Build the backend on the VM or in CI:

```bash
cd backend
cargo build --release
sudo install -o root -g root -m 0755 target/release/firewatch-backend /opt/firewatch/bin/firewatch-backend
```

Install service config:

```bash
sudo install -o root -g root -m 0644 deploy/backend/firewatch-backend.service /etc/systemd/system/firewatch-backend.service
sudo install -o root -g firewatch -m 0640 deploy/backend/backend.env.example /etc/firewatch/backend.env
sudo editor /etc/firewatch/backend.env
sudo systemctl daemon-reload
sudo systemctl enable --now firewatch-backend
```

Watch it:

```bash
journalctl -u firewatch-backend -f
curl -fsS http://127.0.0.1:8080/healthz
```

## Origin HTTPS

If you expose the origin with Caddy, start from `deploy/backend/Caddyfile.example`:

```bash
sudo install -o root -g root -m 0644 deploy/backend/Caddyfile.example /etc/caddy/Caddyfile
sudo editor /etc/caddy/Caddyfile
sudo systemctl reload caddy
```

Prefer Cloudflare Tunnel for the first deployment. If you expose the origin directly later, use a dedicated hostname such as `origin.watch.ferno.ag`, keep it unlisted, firewall SSH tightly, and only expose ports 80/443 publicly.

## R2 Backups

Create an R2 bucket such as `firewatch-backups` and an R2 API token with object read/write access to that bucket.

Install backup files:

```bash
sudo install -o root -g root -m 0755 deploy/backend/backup-sqlite-to-r2.sh /opt/firewatch/bin/backup-sqlite-to-r2.sh
sudo install -o root -g root -m 0644 deploy/backend/firewatch-backup.service /etc/systemd/system/firewatch-backup.service
sudo install -o root -g root -m 0644 deploy/backend/firewatch-backup.timer /etc/systemd/system/firewatch-backup.timer
sudo install -o root -g firewatch -m 0640 deploy/backend/r2-backup.env.example /etc/firewatch/r2-backup.env
sudo editor /etc/firewatch/r2-backup.env
sudo systemctl daemon-reload
sudo systemctl enable --now firewatch-backup.timer
```

Test once:

```bash
sudo systemctl start firewatch-backup.service
journalctl -u firewatch-backup.service -n 80
```

## Next Cost Lever

Large immutable payloads are stored outside SQLite when `BLOB_STORE_DIR` is set. Dokploy mounts `firewatch-blob-data` at `/blobs`; SQLite keeps object keys, hashes, sizes, and metadata. Raw write chunks stay in SQLite for now because storing hundreds of thousands of tiny chunks as individual filesystem objects wastes disk blocks and makes R2 sync noisy.

For live S3/R2 blob storage, set `R2_BLOB_BUCKET` plus credentials, or set the more specific `BLOB_STORE_S3_BUCKET` vars. The backend writes generated program bytes, IDLs, and split-view disassemblies to S3/R2 as soon as they are produced, while keeping `/blobs` as the hot local cache when `BLOB_STORE_DIR` is also set. For Cloudflare R2, use `R2_BLOB_PREFIX=blobs`, set `R2_ACCOUNT_ID` or `BLOB_STORE_S3_ENDPOINT_URL`, and use `AWS_DEFAULT_REGION=auto`.

Current blob layout:

```text
program-bytes/{program}/{sha256}.bin
idl-bytes/{program}/{sha256}.bin
idl-text/{program}/{sha256}.json
disassembly/{program}/{program_sha256}/{asm_sha256}.asm
```

The R2 bucket for off-machine copies is `firewatch-blobs`. If live S3/R2 storage is not enabled, keep the local volume as the hot store for the indexer and sync/copy the blob tree to R2 for backups instead of FUSE-mounting R2 as a live filesystem.

After enabling the volume on an existing SQLite DB, pause the live indexer and run:

```text
firewatch-backend migrate-blobs --vacuum
```

Then schedule the blob copy profile:

```text
docker compose --profile backup run --rm firewatch-r2-blob-sync
```

## First Upgrade Path

Start with one EU writer. Add a US read replica only after traffic proves the need. The cheap version is periodic SQLite snapshot replication from R2 to a US VM; the more robust version is a materialized read database fed from exported event/blob metadata.
