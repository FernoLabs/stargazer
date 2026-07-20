# Stellar archive and live indexing

Status: proposed runbook and architecture, 2026-07-18

## Decision

Do **not** begin by operating a Stellar validator or by setting `CATCHUP_COMPLETE=true`.

For the first Stargazer release:

- use a Galexie-compatible public data lake for historical `LedgerCloseMeta`;
- use a Stellar RPC `getLedgers` source for recent catch-up, development, and a simple live tail;
- use an archive-capable RPC `getLedgers` endpoint for fixtures, bounded historical ranges, and gap recovery;
- use the same official Ingest SDK readers for both paths;
- move the production live tail to Captive Core when independence and predictable low latency justify the operational cost;
- run our own Galexie publisher only when public data-lake availability or retention becomes an unacceptable dependency.

This gives us complete raw ledger metadata without confusing a validator's history archive with an application database.

## “Archive node” means several different things on Stellar

### Full Validator

A Stellar **Full Validator** participates in consensus and publishes a history archive. A Basic Validator does not publish one. Operating either is unnecessary for an application indexer, and publishing history is not the same as exposing indexed application queries.

### History Archive

A history archive is a set of checkpoint files and bucket state published by Full Validators. Captive Core can use these archives to initialize and replay ranges. History archives are excellent network catch-up infrastructure, but they are not a convenient per-ledger query API or our security database.

### Captive Core

Captive Core is a managed `stellar-core` subprocess used by the Ingest SDK. It can replay a bounded historical range or follow newly confirmed ledgers and returns `LedgerCloseMeta`. It is the strongest self-operated live source for this project.

### Galexie data lake

Galexie exports complete ledger metadata in XDR to object storage. It is designed for historical and continuously appended data pipelines. Public Mainnet and Testnet data are currently listed at `s3://aws-public-blockchain/v1.1/stellar/ledgers/`; Lightsail also lists a Mainnet endpoint.

### Stellar RPC retention

RPC is a recent-data service, not a permanent archive by default. `getLedgers` returns ledger headers and `LedgerCloseMeta`, but requests must fall within the provider's retention window. The documented default RPC ledger history window is seven days. `getEvents` defaults to only 24 hours of retained events, with a documented maximum query window of seven days.

RPC v23 can connect `getLedgers` to a Galexie data lake. Providers advertise this as **RPC Archive**. It extends historical access only for `getLedgers`; old `getEvents`, `getTransaction(s)`, and current-state calls remain limited to the provider's hot retention window. This is still sufficient for the indexer because `LedgerCloseMeta` contains the underlying events, transaction results, and state changes.

Therefore, `getEvents` alone cannot power a security indexer. It misses ledger-entry changes, contract creation details, account signer changes, raw Wasm uploads, and any outage longer than its retention.

## Source priority

Use a source abstraction that always yields the same object: `xdr.LedgerCloseMeta`.

| Need | Preferred source | Fallback |
| --- | --- | --- |
| local development | RPC ledger backend | captured fixtures |
| live production | Captive Core | RPC ledger backend |
| short gap recovery | RPC ledger backend | archive RPC `getLedgers` |
| bounded historical replay | archive RPC `getLedgers` | buffered data-lake backend |
| bulk historical replay | buffered data-lake backend | archive RPC `getLedgers` |
| disaster recovery | own immutable data lake | public data lake + verified checkpoint |

Do not implement separate parsing logic for each source. Only acquisition should differ.

## Ingestion protocol

For each ledger:

1. acquire `LedgerCloseMeta` and retain the raw XDR or a content-addressed reference;
2. verify the ledger sequence, ledger hash, and previous-ledger continuity against the committed checkpoint;
3. iterate successful and failed transactions, operations, contract calls, events, diagnostic data needed by rules, and ledger-entry changes;
4. normalize chain coordinates and raw XDR fields without protocol attribution;
5. extract contract/Wasm/account-security facts;
6. run generic security rules and protocol-specific enrichers;
7. insert facts, events, alerts, and the new checkpoint in one database transaction;
8. acknowledge the ledger to the source only after commit.

Failed transactions are valuable security telemetry, but their failed state changes must never be applied to the materialized current-state tables.

## Required invariants

- Ledgers commit strictly in sequence in the live path.
- Historical workers may download or parse in parallel, but database commits preserve deterministic ledger order.
- Every fact has a network and ledger coordinate.
- Reprocessing a ledger is idempotent.
- A conflicting hash for an already committed ledger stops ingestion and raises an operator alert.
- A parser failure stores the raw record and error, then continues unless the ledger itself cannot be decoded.
- Alert delivery happens after the database commit.
- Backfills do not send live alerts unless explicitly enabled.

## Bootstrap strategies

### Fast bootstrap

Use a current checkpoint/state scan to inventory live contract instances and code, then begin live ingestion. This gets a usable monitor online quickly but does not reconstruct creation and upgrade history.

### Complete Soroban history

Replay from the ledger where smart contracts became active on Mainnet and derive every Wasm upload, contract creation, event, instance change, and TTL-related state transition. Discover the starting ledger from protocol-upgrade metadata or a trusted timestamp/ledger service rather than hard-coding an undocumented number.

Recommended sequence:

1. ship the fast bootstrap and live path;
2. backfill complete smart-contract history into staging tables;
3. reconcile staging with live state;
4. atomically mark the historical range complete;
5. run parsers/rules over retained raw XDR with delivery disabled.

There is no security-product need to replay pre-smart-contract Stellar history initially. Classic account history can be added for only the protocol-linked accounts and assets first.

## Contract indexing details

`LedgerCloseMeta` contains transaction results, operation metadata, ledger-entry changes, contract events, upgrades, and eviction information. Use ledger changes as the authoritative materialized-state source.

For each Wasm upload:

- calculate/verify the content hash;
- persist the code once by hash;
- extract `contractenvmetav0`;
- extract optional `contractmetav0` key/value entries;
- extract `contractspecv0` functions, types, events, and errors;
- store the parser and supported environment versions;
- derive interface and metadata diffs against the previous Wasm used by each upgraded instance.

For each contract instance:

- store the `C...` contract ID and executable type;
- link Wasm executables to the immutable Wasm table;
- preserve instance storage separately from code metadata;
- track TTL/live-until information without treating TTL extension as a code upgrade;
- record archive, restore, and eviction state where present.

## Upgrade detection

Use two signals:

1. the `SYSTEM` contract event with topic `executable_update`, which contains the old and new executable references;
2. the contract-instance ledger-entry change, which proves the materialized executable changed.

If only one is observed, store the upgrade but emit an internal consistency finding. The instance diff is authoritative for current state; the system event is the best alert-facing semantic signal.

The new Wasm must already have been uploaded before a normal Wasm upgrade. Treat a new code upload as a lower-severity precursor signal and an instance executable change as the actual upgrade.

## Multisig indexing

Classic Stellar account multisig is stored directly on an account entry: master weight, three operation thresholds, and additional signers. Index the pre/post account entry for all linked admin and treasury accounts. Decode the triggering `SetOptions` operation for intent, but use the resulting ledger entry as current-state truth.

Alert candidates include:

- a threshold decrease;
- signer addition, removal, or weight increase;
- master weight restored from zero or reduced unexpectedly;
- a signer set that can no longer meet a required threshold;
- addition of hash(x) or pre-authorized transaction signers;
- a linked protocol admin account changing home domain or security-relevant flags.

Contract accounts can implement arbitrary authentication, including multisig. There is no universal storage schema. They require verified source/spec classification or curated decoders.

## Operations and observability

Expose at least:

- source type and endpoint/data-lake URI;
- source oldest/latest available ledger;
- last fetched, parsed, and committed ledger;
- network head and ingestion lag in ledgers and seconds;
- last committed ledger hash;
- gap-recovery status;
- per-ledger transaction, event, change, Wasm, contract, and alert counts;
- parser failures by XDR/protocol version;
- raw-XDR retention and blob-store health.

Page immediately when live lag exceeds the recent source's safe recovery margin. The system should switch to data-lake gap fill before resuming the live tail.

## Capacity notes

SDF's full-history Galexie guide estimated roughly 3 TB and about USD 1,100 for a complete full-network export at the time of publication. That estimate is useful for scale, not a fixed budget. Our initial smart-contract-only derived database will be much smaller, but retaining raw full-ledger XDR eventually approaches data-lake scale.

Keep raw ledger metadata in object storage and searchable derived facts in the application database. SQLite is acceptable for the first vertical slice; plan a PostgreSQL migration before sustained parallel historical ingestion or high-cardinality analytics.

## References

- [Stellar indexer options](https://developers.stellar.org/docs/data/indexers)
- [Captive Core ledger backend](https://developers.stellar.org/docs/data/indexers/build-your-own/ingest-sdk/developer_guide/ledgerbackends/captivecore)
- [RPC ledger backend and retention](https://developers.stellar.org/docs/data/indexers/build-your-own/ingest-sdk/developer_guide/ledgerbackends/rpcledgerbackend)
- [RPC provider evaluation](stellar-rpc-provider-evaluation.md)
- [RPC data-lake integration and archive `getLedgers`](https://developers.stellar.org/docs/data/apis/rpc/admin-guide/data-lake-integration)
- [`getLedgers` RPC method](https://developers.stellar.org/docs/data/apis/rpc/api-reference/methods/getLedgers)
- [`getEvents` retention and pagination](https://developers.stellar.org/docs/data/apis/rpc/api-reference/methods/getEvents)
- [Galexie](https://developers.stellar.org/docs/data/indexers/build-your-own/galexie)
- [Public Galexie providers](https://developers.stellar.org/docs/data/indexers/build-your-own/galexie/providers)
- [Full-history Galexie guidance](https://developers.stellar.org/docs/data/indexers/build-your-own/galexie/admin_guide/full-history-exporting)
- [Stellar validator types](https://developers.stellar.org/docs/validators)
- [History archives](https://developers.stellar.org/docs/validators/admin-guide/environment-preparation#history-archives)

