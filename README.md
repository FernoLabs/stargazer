# Ferno Pulsar

Ferno Pulsar is a Stellar security indexer for turning contract deployments, Wasm changes, upgrades, administrative activity, multisig changes, and DeFi runtime behavior into actionable alerts.

Tagline: **Every Stellar contract change, turned into a security signal.**

## Repository status

This repository is a migration baseline created from FireWatch commit `80f4071`. It preserves the useful product history and working alert stack while the Solana-specific ingestion and vocabulary are replaced.

It does **not** index Stellar yet. The current backend and frontend are the inherited FireWatch implementation and still contain Solana-specific code. Keep the source baseline buildable during the migration, but do not deploy it as Ferno Pulsar until the Phase 0 exit criteria in the fork plan are satisfied.

The original FireWatch worktree was not modified or copied with its uncommitted changes. This repository has a read-only-intent `firewatch-source` remote for provenance; configure a new `origin` only after creating the Ferno Pulsar remote repository.

## Architecture direction

```text
Stellar RPC / Captive Core / Galexie
                  |
                  v
      Go Stellar ingestion service
                  |
                  v
       Rust facts, rules, and API
          |                 |
          v                 v
   SQLite/PostgreSQL   Telegram delivery
          |
          v
    SvelteKit dashboard
```

The official Stellar Ingest SDK provides a common `LedgerCloseMeta` model for live and historical sources. The Rust service remains the owner of application persistence, rule evaluation, delivery, and APIs.

## Repository map

```text
backend/        inherited Rust product backend; contains reusable and Solana-only modules
frontend/       inherited SvelteKit product UI; requires contract terminology migration
ingester/       boundary and future Go Stellar ingestion service
deploy/         reusable deployment/backup baseline; names and environment need migration
docs/           Stellar decisions, indexing design, registry research, and migration inventory
scripts/        inherited operational scripts; most are Solana-specific until reviewed
```

## Start here

1. [Stellar fork plan](docs/stellar-fork-plan.md)
2. [Archive and live indexing architecture](docs/stellar-archive-and-live-indexing.md)
3. [Contract registry research](docs/stellar-contract-registry-research.md)
4. [DeFi backfill parser and incident fixture plan](docs/stellar-defi-backfill-parser-plan.md)
5. [Migration inventory](docs/stellar-migration-inventory.md)
6. [SCF grant-readiness plan](docs/stellar-scf-grant-readiness.md)
7. [SCF Build application draft](docs/stellar-scf-application-draft.md)
8. [Ingestion service boundary](ingester/README.md)

## First vertical slice

The first implementation milestone is deliberately small:

1. ingest recent Mainnet ledgers from an RPC `getLedgers` source;
2. discover contract creation and Wasm uploads;
3. extract contract spec and metadata;
4. detect an `executable_update` system event and confirm the instance change;
5. display and send one upgrade alert;
6. replay the same range and prove idempotency.

Only after this path works should the project begin a complete smart-contract history replay.

## Source baseline checks

These commands validate the inherited application while it is being migrated:

```bash
cd backend && cargo test
cd frontend && npm ci && npm run check && npm run build
```

The inherited backend currently requires Solana configuration to run. That requirement is removed as part of Phase 0; it is not a Ferno Pulsar runtime requirement.

## License

MIT
