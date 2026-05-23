# FireWatch

FireWatch is a Solana monitoring stack for tracking program upgrades, Squads multisig activity, SNS `.sol` domain changes, and curated DeFi protocol signals.

It has two main parts:

- `backend/`: Rust indexer and API backed by SQLite.
- `frontend/`: SvelteKit dashboard for live events, protocol views, and alert management.

FireWatch can also send Telegram alerts for watched programs, multisigs, DeFi protocol activity, and metric threshold rules.

## Repository

```text
backend/        Rust indexer, API, SQLite models
frontend/       SvelteKit dashboard and Cloudflare Pages proxy
deploy/         Dokploy, Docker, backup, and systemd helpers
docs/           Deployment notes
backlog/        Product planning notes
```

## Local Development

```bash
cp backend/.env.example backend/.env
cp frontend/.env.example frontend/.env
```

Fill in the backend Solana gRPC settings, then run:

```bash
cd backend
cargo run --release
```

In another shell:

```bash
cd frontend
npm install
npm run dev
```

## Checks

```bash
cd backend && cargo test
cd frontend && npm run check
cd frontend && npm run build
```

## Deployment

See [docs/deployment.md](docs/deployment.md) for the Cloudflare Pages frontend, Dokploy backend, and R2 backup runbook.

## License

MIT
