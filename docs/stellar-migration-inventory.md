# Stellar migration inventory

Status: initial source classification, 2026-07-18

This inventory identifies what is useful in the FireWatch baseline and prevents two common mistakes: deleting mature product behavior too early, or treating chain-specific code as reusable merely because it is written cleanly.

## Baseline verification

The inherited commit was verified in this repository on 2026-07-18:

- `cargo test`: 152 passed, 0 failed;
- `npm run check`: 0 errors and 0 warnings;
- `npm run build`: successful Cloudflare production build;
- `npm ci`: successful, with 6 inherited audit findings (1 low and 5 high) requiring a separate dependency review.

Do not run an automatic audit fix as part of the chain migration. Review dependency upgrades separately so frontend regressions are attributable.

## Migration rule

Keep the inherited application buildable while replacements land. Remove a Solana subsystem only when its Stellar replacement has a fixture test and an equivalent operator-visible outcome, or when the product explicitly no longer needs the feature.

## Reuse as product capability

These capabilities are worth preserving, though file-level extraction may still be required:

- event persistence, pagination, review state, and deterministic delivery state;
- Telegram live delivery, retry behavior, commands, subscriptions, and daily summaries;
- entitlement, promo-code, account, and admin workflows;
- blob storage and R2/S3 backup behavior;
- Axum HTTP serving, health checks, CORS, origin authentication, and admin authentication;
- link previews and alert preview-card generation;
- deployment containers, Caddy patterns, service supervision, and database/blob backup runbooks;
- frontend API proxy policy, live-status context, event feeds, protocol navigation, admin surfaces, and responsive layout primitives.

Reuse the behavior, tests, and UX—not necessarily the current table or type names.

## Reuse after chain-neutral refactoring

### Backend

| Surface | Useful behavior | Required change |
| --- | --- | --- |
| `models.rs` | event severity, scopes, delivery modes | replace program/slot/signature assumptions with contract/ledger/transaction coordinates |
| `database.rs` | mature persistence and query behavior | split generic product tables from Solana program, IDL, SNS, pool, and payment tables |
| `http.rs` | APIs, auth, pagination, admin behavior | rename program resources, remove Solana payment/explorer assumptions, expose Stellar health |
| `delivery.rs` | retry and summary orchestration | change event rendering and explorer links |
| `telegram.rs` | bot lifecycle and configuration UX | change commands from program/Squads language to contract/account security language |
| `blob_store.rs` | content-addressed large-object persistence | store immutable Stellar Wasm and raw ledger metadata |
| `program_bytes.rs` | hashing and binary comparison ideas | replace BPF-specific assumptions with Wasm hashing and sections |
| `program_disassembly.rs` | version-diff product concept | replace sBPF disassembly with Wasm-aware inspection or defer disassembly |
| `protocols.rs` | protocol grouping and coverage levels | replace curated Solana program IDs with evidence-backed Stellar contract mappings |

### Frontend

Preserve the application shell, server-side backend proxy, live event stream, filters, protocol views, alert configuration, program-version comparison concept, admin workflows, and generic design components.

Rename the public model in one pass:

- `/programs` becomes `/contracts`;
- program address becomes contract ID;
- slot becomes ledger;
- signature becomes transaction hash;
- IDL/interface becomes contract spec and metadata;
- Squads multisig becomes account security or contract-account governance depending on the evidence;
- Solana explorer links become network-configured Stellar explorer links.

Do not mechanically rename `ProgramVersionDiff` until the new Wasm version model is defined. The component behavior is useful, but sBPF assembly and Anchor instruction diffs are not.

## Replace

These subsystems encode Solana execution semantics and should receive new Stellar implementations:

- `backend/src/solana.rs` and Yellowstone/Geyser subscription handling;
- `backend/src/block_backfill.rs` and Ferno/Solana RPC block acquisition;
- BPF loader and Loader v4 deployment/upgrade parsing;
- Anchor IDL account discovery, decoding, and instruction catalog generation;
- Squads proposal, approval, vault execution, and authority derivation;
- SPL Governance realm discovery;
- SNS `.sol` name-service parsing and bootstrap;
- Solana account/PDA derivation and explorer URL construction;
- Solana Pay checkout and SPL-token payment verification;
- Solana token-account, mint, pool-vault, and protocol instruction decoders;
- Solana-specific program authority, program metadata, and disassembly models.

Their Stellar replacements are based on `LedgerCloseMeta`, contract/Wasm ledger entries, contract system events, contract specs, classic account entries, authorization trees, and curated contract storage decoders.

## Quarantine until a product decision

Do not delete these immediately, but do not let them define the new schema:

- token liquidity and reserve-drain rules;
- protocol metric observations and threshold presets;
- wallet-to-protocol inference;
- paid checkout implementation;
- community upgrade review;
- binary disassembly and patch presentation.

Each remains a useful product idea. Reintroduce it after the Stellar contract, token, and account models are stable.

## Scripts and snapshots

The inherited `scripts/` directory and `backend/data/` snapshots are primarily Solana-specific. Keep them during migration for behavioral reference, then remove them from the active runtime once Stellar registry synchronization exists.

New registry imports must stage candidates with source, fetch time, confidence, and validity range. No remote list should directly overwrite the canonical protocol-to-contract mapping.

## Deployment migration

Reuse Docker, Dokploy, systemd, Caddy, SQLite backup, and R2 synchronization patterns. Change before deployment:

- service/image names from FireWatch to Ferno Pulsar;
- public and origin hostnames;
- environment variable names and examples;
- Solana provider and payment configuration;
- database and blob prefixes;
- health metrics from slot lag to ledger lag;
- frontend Cloudflare project and cache keys;
- Telegram bot identity and links.

Never point the fork at the production FireWatch database, bot token, blob bucket prefix, or hostname.

## Removal gate

The inherited Solana runtime can be removed when all of the following are true:

- the backend starts without Solana environment variables;
- a captured Stellar ledger fixture crosses the ingestion boundary;
- ledger checkpoint and replay idempotency tests pass;
- contract creation, Wasm upload, metadata extraction, and upgrade detection are persisted;
- one upgrade alert renders in the API, frontend, and Telegram;
- the old FireWatch test suite has been classified as migrated, intentionally dropped, or retained as generic coverage.
