# Stellar ingestion service

This directory is the boundary for the future Go ingestion service based on the official Stellar Ingest SDK.

No ingestion executable has been added yet. The first code change here should be driven by a captured `LedgerCloseMeta` fixture so the boundary is defined by real Stellar data instead of a speculative abstraction.

## Responsibilities

The ingestion service will:

- acquire `LedgerCloseMeta` from an RPC ledger backend, Captive Core, or a Galexie-compatible data lake;
- verify basic ledger coordinates and source availability;
- use the same readers for historical and live ledgers;
- normalize transactions, operations, contract events, and ledger-entry changes;
- submit strictly versioned ledger envelopes to the private Rust ingestion boundary;
- retry transport failures without advancing its acknowledged checkpoint;
- expose acquisition and parsing metrics.

It will not:

- write the application database directly;
- evaluate product severity or send alerts;
- treat an external contract registry as chain truth;
- maintain a checkpoint that can advance independently of the Rust database commit;
- silently skip XDR it cannot parse.

## First implementation order

1. Add one captured Mainnet `LedgerCloseMeta` fixture containing a Wasm upload or contract creation.
2. Pin the Stellar Go SDK version and record the supported XDR/protocol versions.
3. Implement RPC acquisition with explicit oldest/latest ledger reporting.
4. Define a versioned envelope from the decoded fixture.
5. Add the private Rust endpoint and atomic checkpoint commit.
6. Prove duplicate replay and deliberate gap recovery.
7. Add buffered data-lake and Captive Core sources behind the same interface.

## Proposed configuration

See [`.env.example`](.env.example). Configuration names are provisional until the first executable lands.

## POC source decision

The initial provider rotation was verified against live Mainnet on 2026-07-18:

- use Gateway for the live/recent ledger tail;
- fail over live acquisition to Ankr, then Lightsail;
- use Ankr archive `getLedgers` for bounded historical ranges and gap recovery;
- fail over archive acquisition to Lightsail's separate archive endpoint;
- use the public AWS Galexie lake directly for bulk replay.

Archive RPC extends only `getLedgers`. The ingester must extract historical events and state changes from the returned `LedgerCloseMeta`; it must not expect old `getEvents` or `getTransactions` calls to work.

Probe the public endpoints with:

```bash
./scripts/probe-rpc-providers.sh
```

The default historical probe is ledger `50,859,939`. See the [provider evaluation](../docs/stellar-rpc-provider-evaluation.md) for observed results and the first real `executable_update` fixture candidate at ledger `63,416,408`.

## DeFi parser track

The checked-in [Mainnet DeFi root seed](data/stellar-mainnet-defi-roots.json) contains 36 source-attributed monitoring roots/deployments across seven protocols plus two incident contracts. It is intentionally incomplete without factory and historical-event expansion. The [backfill parser plan](../docs/stellar-defi-backfill-parser-plan.md) defines the protocol adapters, authorization model, and the first incident fixture.

The seven checkpoint ledgers `61,339,940`, `61,340,263`, `61,340,315`, `61,340,381`, `61,340,384`, `61,340,405`, and `61,340,408` reproduce the known YieldBlox SDEX-to-oracle-to-Blend transactions. Actual anomaly calculations also require a continuous range through at least `61,340,450` and a trusted active-ledger state snapshot immediately before the range, because pre-existing offers and contract state are not recreated by isolated ledger responses. Historical replay must retain findings with alert delivery disabled.

Fetch the ground-truth ledger responses with:

```bash
./scripts/fetch-ledger-fixtures.sh
```

Capture the continuous transaction window separately (this can be much larger):

```bash
./scripts/fetch-ledger-fixtures.sh ./fixtures/yieldblox-range --range 61339940 61340450
```

The seven sampled responses totaled about 15 MB as JSON. A linear estimate for this 511-ledger window is roughly 1.1 GB before filesystem compression, but actual ledger sizes vary; keep generated fixtures outside Git and place the full range on the NAS if local space is tight.

Pass an output directory followed by explicit ledger sequences to select other checkpoints. Set `STELLAR_ARCHIVE_RPC_URL` to use a different archive provider. The script records that a range needs a starting-state snapshot; fetching that state is a separate Captive Core/Galexie POC task.
