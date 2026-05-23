# FireWatch

FireWatch is a Solana monitoring stack with two parts:

- `backend/`: a Rust indexer that subscribes to Yellowstone gRPC, parses program upgrades, Squads multisig activity, SNS `.sol` domain events, and curated DeFi protocol signals, stores every event in SQLite, and retries Telegram delivery.
- `frontend/`: a SvelteKit dashboard that renders the live event feed, active Telegram rules, and the curated protocol catalog.

## What It Tracks

The backend subscribes to finalized transactions that mention:

- Solana loaders:
  - legacy loader
  - deprecated loader
  - upgradeable loader
  - loader v4
- Squads v4 multisig:
  - `proposal_activate`
  - `proposal_approve`
  - `proposal_reject`
  - `proposal_cancel` / `proposal_cancel_v2`
  - `vault_transaction_execute`
  - `config_transaction_execute`
- Curated DeFi programs:
  - program upgrades for every cataloged protocol surface
  - known admin instructions where we have instruction catalogs
  - unknown instruction review alerts where deep decoding is available
- SNS / `.sol` domains:
  - domain create
  - domain update
  - domain ownership transfer
  - domain delete
  - domain realloc

The backend also keeps a local SNS index in SQLite for discovered domain accounts and wallet primary domains, so Telegram alerts and the frontend can show `.sol` names instead of only raw pubkeys.

The parser also inspects inner instructions, so a program upgrade or protocol action executed through Squads can emit both:

- the Squads execution event
- the underlying decoded program event

## Architecture

1. The Rust service subscribes to Yellowstone gRPC with finalized commitment.
2. Matching events are normalized and inserted into backend SQLite.
3. Telegram subscriptions are matched against the event stream.
4. Each Telegram delivery gets its own retry record in SQLite.
5. A background SNS sync task bootstraps recent addresses into the local `.sol` index.
6. The frontend reads the canonical backend event feed and subscription state.

The backend is the source of truth for:

- the event log
- the last processed slot
- Telegram subscriptions
- Telegram retry state

## Local Setup

Copy the example environment files:

```bash
cp backend/.env.example backend/.env
cp frontend/.env.example frontend/.env
```

Fill in at least:

- `backend/.env`
  - `SOLANA_GRPC_ENDPOINT`
  - `SOLANA_GRPC_X_TOKEN` if your provider gives you a separate gRPC token
  - `SOLANA_RPC_HTTP_URL` if you want explicit JSON-RPC rather than using the gRPC endpoint host/token pattern
  - `SOLANA_EXPLORER_CLUSTER` if alert links should point to `devnet` or `testnet`
  - `TELEGRAM_BOT_TOKEN`
  - `SNS_REFRESH_SECONDS` if you want to change how often FireWatch syncs recent addresses into the local `.sol` index
  - `CRITICAL_SOL_DOMAINS` if some root domains should always be treated as sensitive reviews
- `frontend/.env`
  - `BACKEND_BASE_URL`
  - `ADMIN_PASSWORD` if you want to use `/admin` locally
  - `BACKEND_ORIGIN_TOKEN` if backend `ORIGIN_API_TOKEN` is set

For Triton specifically, the backend accepts either of these forms:

- split credentials:
  - `SOLANA_GRPC_ENDPOINT=https://your-endpoint.mainnet.rpcpool.com`
  - `SOLANA_GRPC_X_TOKEN=your-secret-token`
- tokenized endpoint:
  - `SOLANA_GRPC_ENDPOINT=https://your-endpoint.mainnet.rpcpool.com/your-secret-token`
  - leave `SOLANA_GRPC_X_TOKEN` empty

## Telegram Setup

The bot must already be able to write into the target chat, group, topic, or channel.

The dashboard lets you create backend-managed Telegram subscriptions for:

- all program updates: `scope=program_updates` and leave `target_address` empty
- one program only: `scope=program_updates` and set `target_address` to the program pubkey
- all multisig activity: `scope=multisig_activity` and leave `target_address` empty
- one multisig only: `scope=multisig_activity` and set `target_address` to the multisig pubkey
- curated DeFi watch: `scope=defi_watch` plus optional protocol and instruction tags

`chat_id` can be a numeric chat id such as `-100...` or a Telegram channel username the bot can post into. `thread_id` is optional and is only needed for forum topics.

If `TELEGRAM_BOT_TOKEN` is configured, the backend also listens for simple Telegram commands through `getUpdates`:

- `/help` or `/start`: show the available bot commands
- `/info`: show current slot, parsed tx count, matched tx count, and active rules in the chat
- `/list`: show active subscriptions in the current chat
- `/watch all`: subscribe the current chat to all new program updates, multisig activity, and curated DeFi signals
- `/watch defi`: subscribe the current chat to curated major Solana DeFi upgrade and admin alerts
- `/unwatch all`: remove all rules in the current chat

The commands are idempotent for the same chat and thread, and they only apply to new events after the command is accepted.

## HTTP Surfaces

Backend:

- `GET /healthz`
- `GET /api/events?limit=120&offset=0`
- `GET /api/status`
- `GET /api/protocols`
- `GET /api/programs/:address/signer-stats`
- `GET /api/programs/:address/versions`
- `GET /api/programs/:address/versions/:version_id/bytes?side=after|before`
- `GET /api/programs/:address/versions/:version_id/disassembly?side=after|before`
- `GET /api/telegram-subscriptions`
- `GET /api/telegram-subscriptions/summary`
- `POST /api/telegram-subscriptions`
- `DELETE /api/telegram-subscriptions/:id`
- `GET /api/metric-alerts`
- `POST /api/metric-alert-rules`
- `DELETE /api/metric-alert-rules/:id`

Frontend:

- `GET /api/health`

## Run It

Backend:

```bash
cd backend
cargo run --release
```

Frontend:

```bash
cd frontend
npm run dev
```

## Verification

```bash
cd backend && cargo test
cd frontend && npm run check
cd frontend && npm run build
```

## Deployment

See [docs/deployment.md](docs/deployment.md) for the Cloudflare frontend, Dokploy indexer origin, optional systemd fallback, and R2 backup runbook.
