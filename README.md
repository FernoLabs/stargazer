# Stargazer

Stargazer by Ferno is a Stellar protocol intelligence project for monitoring contract
deployments, Wasm upgrades, governance, administrative activity, repositories, SDKs,
packages, dependencies, wallets, and DeFi runtime behavior from one evidence-backed feed.

> Know when a Stellar dependency changes, why it matters, and what proves it.

## Current status

This repository is pre-POC. It does not yet run a Stellar indexer or production alerting
service.

The active tree contains:

- the deployable Stargazer product website for future developers and protocol teams;
- an unlisted, non-indexed SCF application working page;
- Stellar archive, RPC, registry, multisig, and parser research;
- provider-probing and ledger-fixture acquisition scripts;
- a source-attributed Stellar Mainnet DeFi registry seed.

Only Stellar-facing product and research work belongs in this repository. Prior runtime
implementations remain available through Git history but are not part of the active
Stargazer codebase.

## Architecture direction

```text
Stellar RPC / Captive Core / Galexie
                  |
                  v
      Go Stellar ingestion service
                  |
                  v
      Versioned ledger fact envelope
                  |
                  v
       Facts, rules, storage, and API
          |                 |
          v                 v
      Dashboard       Alert delivery
```

The official Stellar Ingest SDK provides the common `LedgerCloseMeta` model for live
and historical sources. Acquisition, normalization, persistence, rule evaluation, and
delivery remain separate trust boundaries.

## Repository map

```text
frontend/       SvelteKit product site and SCF working page
ingester/       Stellar ingestion boundary, provider probes, and registry seed
docs/           Architecture, provider, protocol, parser, and grant research
```

## Start here

1. [Archive and live indexing architecture](docs/stellar-archive-and-live-indexing.md)
2. [RPC provider evaluation](docs/stellar-rpc-provider-evaluation.md)
3. [Contract registry research](docs/stellar-contract-registry-research.md)
4. [DeFi backfill parser and incident plan](docs/stellar-defi-backfill-parser-plan.md)
5. [Ingestion service boundary](ingester/README.md)
6. [SCF grant-readiness plan](docs/stellar-scf-grant-readiness.md)
7. [SCF Build application draft](docs/stellar-scf-application-draft.md)

## First vertical slice

The first implementation milestone is deliberately narrow:

1. ingest a captured Mainnet ledger range;
2. discover contract creation and Wasm uploads;
3. extract contract spec and metadata;
4. detect an `executable_update` system event and confirm the instance change;
5. expose one evidence-backed upgrade finding;
6. replay the same range and prove idempotency.

Only after this path works should the project begin complete smart-contract history
replay.

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

## License

MIT
