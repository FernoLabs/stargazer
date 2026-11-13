# Stargazer

Stargazer by Ferno tells you when a Soroban contract or a protocol dependency changes on
Stellar, why that change matters, and which ledger evidence proves it. It keeps the control
history of every observed contract (which code runs, since when, who can change it), applies
deterministic detectors to every Mainnet ledger, delivers findings over Telegram and signed
webhooks, and turns a major finding into an on-chain protective action through GuardVault.

> Know when a Stellar dependency changes, why it matters, and what proves it. Then act on it.

## Current status

This repository contains a working archive-index and wallet-monitoring proof of concept. It is
not yet a production alerting service: continuous ingestion, live alert delivery and the
protection vault are the work described in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

What runs today, on a local Galexie archive of Stellar Mainnet (6,336,000 ledgers, 28 May 2025
to 23 July 2026):

- a bounded-memory Rust archive reader, a compact SQLite evidence index and a read-only API;
- the executable history of every contract, 36 protocol root contracts attributed across 7
  families, a watcher over 8 classic controller accounts;
- the bounded replay of the YieldBlox incident of 22 February 2026;
- the product website and the wallet dependency dashboard;
- a separate Telegram pairing and alert-policy control plane.

Only Stellar-facing product and research work belongs in this repository.

## What we are building

The same pipeline, fed continuously from Mainnet and exposed publicly:

| Block | Component |
|---|---|
| Live Mainnet ingestion over Stellar RPC, continuity checks, atomic commit, catch-up from the archive | LiveLedgerSource, ContinuityVerifier, AtomicCommit, ArchiveReplay |
| Public control history per Soroban contract: Wasm lineage, reference version, admin, signers, thresholds, "changed since reference" | ContractControlHistory, AttributionRegistry |
| Generic decoding of any contract that embeds its SEP-48 specification or emits the standard OpenZeppelin Stellar events; one dedicated decoder for Blend | GenericDecoder, BlendDecoder |
| Public detector catalogue: identifier, rule, severity, Mainnet fixture, replay test; AI context that explains and never decides | DetectorCatalogue, AIContext |
| Subscriptions by protocol, contract or wallet at a chosen level; idempotent Telegram delivery; signed JSON webhooks for explorers, wallets and rules engines | WatchSubscriptions, TelegramOutbox, WebhookOutbox |
| Versioned API, public dashboard, status page with the exact coverage boundary | EvidenceAPI, StatusPage |
| Non-custodial USDC vault on the OpenZeppelin Stellar libraries, allowed DeFindex venues, caps, and an off-chain guardian that pulls a position back to idle on a major finding | GuardVault, Sentinel |
| Real hosting, failover, backup, restore drills, runbook | ProductionOperations |

The full design, the detector catalogue, the YieldBlox replay, the vault roles and caps, the
threat model and the monitoring plan are in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Architecture overview

```text
Stellar RPC / archive RPC / Galexie data lake
                  |
                  v
   Live ingestion, continuity check, atomic commit
                  |
                  v
   Contract control history, protocol attribution
                  |
                  v
   Generic SEP-48 and OpenZeppelin decoding, Blend decoder
                  |
                  v
   Detector catalogue (severity + ledger fixture), AI context
          |                 |                  |
          v                 v                  v
   API, dashboard,    Telegram and       Sentinel (guardian)
   status page        signed webhooks          |
                                               v
                                    GuardVault (USDC, capped)
                                               |
                                               v
                                    DeFindex USDC vaults
```

Acquisition, normalization, persistence, rule evaluation, delivery and the vault remain
separate trust boundaries. A historical replay never sends an alert and never triggers an
on-chain action.

## Repository map

```text
frontend/       SvelteKit product site, dashboard, and /docs
ingester/       Archive indexer, read API, alert control, provider probes, and registry seed
cloudflare/     Private gateway between the website and the read API
benchmarks/     Archive sample and soak manifests
docs/           Technical architecture, provider, protocol, and parser research
```

## Start here

1. [Technical architecture](docs/ARCHITECTURE.md)
2. [Archive and live indexing](docs/stellar-archive-and-live-indexing.md)
3. [RPC provider evaluation](docs/stellar-rpc-provider-evaluation.md)
4. [Contract registry research](docs/stellar-contract-registry-research.md)
5. [DeFi backfill parser and incident plan](docs/stellar-defi-backfill-parser-plan.md)
6. [Ingestion service boundary](ingester/README.md)

The research notes date from the proof of concept (July to August 2026). Where a note and the
architecture differ, the architecture applies.

## Current vertical slice

The implemented proof-of-concept slice is deliberately narrow:

1. ingest a captured Mainnet ledger range;
2. discover contract creation and Wasm uploads;
3. extract contract spec and metadata;
4. detect an `executable_update` system event and confirm the instance change;
5. expose one evidence-backed upgrade finding;
6. replay the same range and prove idempotency.

The wallet product builds exact wallet-to-contract coverage and retrospective incident findings
from this foundation. Telegram account pairing and policy persistence are separate from the
archive databases; the live event dispatcher, the signed webhook, the public control-history
registry and GuardVault are the next steps.

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
The public product domain is [`stargazer.ferno.ag`](https://stargazer.ferno.ag/). The Pages
project keeps its original name, `ferno-pulsar`; its `pages.dev` host redirects to the product
domain.

The `/docs/architecture` page renders `docs/ARCHITECTURE.md` at build time. Edit that file, not
the page.

Public releases start from a reviewed snapshot with clean Git history. See the
[public release checklist](docs/public-release-checklist.md). Report security issues through
[SECURITY.md](SECURITY.md).

## License

MIT
