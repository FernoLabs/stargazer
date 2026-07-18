# Stellar RPC provider evaluation

Status: POC source decision, verified 2026-07-18

## Decision

Use three acquisition tiers that all yield the same `LedgerCloseMeta` object:

1. **Live and recent ledgers:** Gateway public Mainnet RPC, with Ankr as the first failover.
2. **Bounded historical ranges and gap recovery:** archive `getLedgers` through Ankr, with Lightsail Quasar as the first failover.
3. **Bulk historical replay:** the anonymous AWS Galexie data lake in `us-east-2`.

Public RPC is appropriate for the POC, not a production SLA. Production must use authenticated endpoints or self-operated infrastructure, retain source-independent checkpoints, and fail over without changing parsing logic.

## POC configuration

```dotenv
STELLAR_RPC_URL=https://rpc.mainnet.stellar.gateway.fm
STELLAR_RPC_FALLBACK_URLS=https://rpc.ankr.com/stellar_soroban,https://rpc.lightsail.network/

STELLAR_ARCHIVE_RPC_URL=https://rpc.ankr.com/stellar_soroban
STELLAR_ARCHIVE_RPC_FALLBACK_URLS=https://archive-rpc.lightsail.network/,https://stellar.api.onfinality.io/public

STELLAR_DATA_LAKE_URI=s3://aws-public-blockchain/v1.1/stellar/ledgers/pubnet
STELLAR_DATA_LAKE_REGION=us-east-2
```

Do not send a complete backfill through a public RPC endpoint. Archive RPC is convenient for fixtures, bounded replays, and outage recovery. Direct Galexie access is the bulk path.

## What was verified

Read-only probes were run against public Mainnet endpoints on 2026-07-18.

| Provider | Live health and recent `getLedgers` | `getLedgers(50,859,939)` | POC use |
| --- | --- | --- | --- |
| Gateway | passed | passed | primary live; Soroban-era archive fallback |
| Ankr | passed | passed | first live failover and primary archive RPC |
| OnFinality | passed | passed | archive/live fallback with a documented paid path |
| Lightsail live | passed | correctly rejected the old ledger | live fallback |
| Lightsail archive | passed | passed | archive fallback |
| sorobanrpc.com | passed | correctly rejected the old ledger | extra live fallback only |
| Nodies | returned HTTP 503 maintenance | not tested | exclude from the initial rotation |
| Liquify URL listed by Stellar | returned `404 Chain not found` | not tested | exclude until its public URL is corrected |

All six healthy endpoints returned the same recent ledger hash for ledger `63,535,080`:

```text
e4aa71e952faf46ef53239b239f6cd8d909c03b8b94b3058fb0e960dc48eb642
```

Gateway, Ankr, OnFinality, and Lightsail archive returned the same historical hash for ledger `50,859,939`:

```text
be8a973e4f8272f8733ad545b4e73ef411e72599946a9679c9cc9c3e9357c487
```

Ankr and Lightsail archive also returned ledger 2. Gateway and OnFinality returned an object-missing error for ledger 2 during this probe, although both worked for Soroban-era history. Ferno Pulsar does not need pre-Soroban history for its first milestone.

Re-run the same checks with:

```bash
./ingester/scripts/probe-rpc-providers.sh
```

An optional first argument changes the historical ledger used by the probe.

## API boundary

Standard Stellar RPC retains a provider-configured recent window, commonly seven days. Archive mode extends **only** `getLedgers`; it does not make old `getEvents`, `getTransaction`, `getTransactions`, or state queries historical.

This is sufficient for Ferno Pulsar because `getLedgers` returns the ledger header and complete `LedgerCloseMeta`, including transactions, results, events, and ledger-entry changes. The ingester must decode that XDR rather than depend on historical `getEvents` calls.

The source router should:

1. call `getNetwork` and reject the wrong passphrase;
2. inspect live `oldestLedger` and `latestLedger` at runtime;
3. use live RPC when the requested sequence is inside that range;
4. use archive RPC for a bounded older range or live gap;
5. use the Galexie backend for bulk replay;
6. compare network, sequence, ledger hash, previous hash, and close time before accepting a source transition.

An archive endpoint's `getHealth.oldestLedger` describes its local hot store, not necessarily the beginning of its data-lake-backed `getLedgers` coverage. Archive capability must be tested with an actual old-ledger request.

## First real upgrade fixture

A seven-day Mainnet `getEvents` query found four `executable_update` system events. Use this one for the first end-to-end fixture:

```text
ledger:      63,416,408
contract:    CBMUXOGMG724UVTP54JGR447IDGA47NSRASBXQIWSEFEYB45YC4BIRIT
transaction: 0351d70bcfb816137fa2ed970daf3e00b1b932d759fa0c20f8002cfacf5e2200
event topic: executable_update
```

The fixture workflow is:

1. acquire ledger `63,416,408` through archive RPC;
2. save and decode its `metadataXdr` as `LedgerCloseMeta`;
3. assert that the transaction and system event above are present;
4. normalize old/new executable references;
5. replay the fixture twice and prove a single committed upgrade fact.

## Provider landscape

The current Stellar directory marks Gateway, Ankr, Obsrvr, OnFinality, Lightsail Quasar, Exaion, and GetBlock as archive-capable. Blockdaemon, Validation Cloud, QuickNode, NowNodes, Infstones, Nodies, Uniblock, and Alchemy are listed for live Mainnet RPC but not archive RPC.

Commercial services remain useful after the POC:

- OnFinality has a public development endpoint and authenticated paid plans.
- Obsrvr offers RPC plus decoded Flow/Lake products, but is not needed to prove our own ingestion path.
- GetBlock and Exaion require an account or sales process.

Do not make the application aware of a provider brand. Configuration supplies ordered live, archive, and data-lake sources; the ledger/checkpoint invariants remain provider-neutral.

## References

- [Official Stellar RPC provider directory](https://developers.stellar.org/docs/data/apis/rpc/providers)
- [`getLedgers` API](https://developers.stellar.org/docs/data/apis/rpc/api-reference/methods/getLedgers)
- [RPC ledger backend and retention](https://developers.stellar.org/docs/data/indexers/build-your-own/ingest-sdk/developer_guide/ledgerbackends/rpcledgerbackend)
- [RPC data-lake integration](https://developers.stellar.org/docs/data/apis/rpc/admin-guide/data-lake-integration)
- [AWS Galexie providers](https://developers.stellar.org/docs/data/indexers/build-your-own/galexie/providers)
- [Ankr Stellar RPC](https://www.ankr.com/docs/rpc-service/chains/chains-api/stellar/)
- [OnFinality Stellar RPC](https://documentation.onfinality.io/support/stellar)
- [Lightsail Quasar](https://quasar.lightsail.network/)
