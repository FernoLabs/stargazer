# Stargazer

Stargazer by Ferno is a Stellar protocol intelligence project for monitoring contract
deployments, Wasm upgrades, governance, administrative activity, repositories, SDKs,
packages, dependencies, wallets, and DeFi runtime behavior from one evidence-backed feed.

> Know when a Stellar dependency changes, why it matters, and what proves it.

## Current status

This repository contains a working archive-index and wallet-monitoring POC. It is not yet a
production alerting service.

The active tree contains:

- the deployable Stargazer product website and real-data wallet dashboard;
- a bounded-memory Rust archive reader, compact SQLite reverse index, and read-only API;
- a separate Telegram pairing and alert-policy control plane with fail-closed billing;
- Stellar RPC, registry, multisig, parser, and incident research;
- a source-attributed Stellar Mainnet DeFi registry seed and governance watcher.

Only Stellar-facing product and research work belongs in this repository. Prior runtime
implementations and private planning material are not part of this public codebase.

## Architecture direction

```text
Stellar RPC / Captive Core / Galexie
                  |
                  v
       Archive / live acquisition
                  |
                  v
      Versioned ledger fact envelope
                  |
                  v
      Facts, rules, compact index
          |                 |
          v                 v
    Read-only API      Alert control DB
          |                 |
          v                 v
      Dashboard        Telegram bot
```

The official Stellar Ingest SDK provides the common `LedgerCloseMeta` model for live
and historical sources. Acquisition, normalization, persistence, rule evaluation, and
delivery remain separate trust boundaries.

## Repository map

```text
frontend/       SvelteKit product site and dashboard
ingester/       Archive indexer, read API, alert control, provider probes, and registry seed
docs/           Architecture, provider, protocol, and parser research
```

## Start here

1. [Archive and live indexing architecture](docs/stellar-archive-and-live-indexing.md)
2. [RPC provider evaluation](docs/stellar-rpc-provider-evaluation.md)
3. [Contract registry research](docs/stellar-contract-registry-research.md)
4. [DeFi backfill parser and incident plan](docs/stellar-defi-backfill-parser-plan.md)
5. [Ingestion service boundary](ingester/README.md)

## Current vertical slice

The implemented POC slice is deliberately narrow:

1. ingest a captured Mainnet ledger range;
2. discover contract creation and Wasm uploads;
3. extract contract spec and metadata;
4. detect an `executable_update` system event and confirm the instance change;
5. expose one evidence-backed upgrade finding;
6. replay the same range and prove idempotency.

The wallet product now builds exact wallet-to-contract coverage and retrospective incident
findings from this foundation. Telegram account pairing and policy persistence are separate from
the archive databases; a live event dispatcher and paid settlement rail remain future work.

## Website development

```bash
cd frontend
npm ci
npm run dev
```

Verification:

```bash
cd frontend
npm run check
npm run build

bash -n ../ingester/scripts/fetch-ledger-fixtures.sh
bash -n ../ingester/scripts/probe-rpc-providers.sh
jq empty ../ingester/data/stellar-mainnet-defi-roots.json
```

Cloudflare Pages deployment runs through
[`.github/workflows/cloudflare-pages.yml`](.github/workflows/cloudflare-pages.yml).
The public product domain is [`stargazer.ferno.ag`](https://stargazer.ferno.ag/).
The previous `stellarwatch.ferno.ag` hostname remains available as a legacy alias.

Public releases must start from a reviewed snapshot with clean Git history. See
the [public release checklist](docs/public-release-checklist.md). Report security
issues through [SECURITY.md](SECURITY.md).

## License

MIT
