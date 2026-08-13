# Stellar ingestion service

This directory is the boundary for the future Go ingestion service based on the official Stellar Ingest SDK.

The production ingestion service is not wired yet. The first reader change is
driven by a real captured `LedgerCloseMetaBatch` fixture so the boundary is
defined by actual Stellar data instead of a speculative abstraction.

The first Rust archive indexer is available in [`reader-rs`](reader-rs/). It
validates Galexie `LedgerCloseMetaBatch` files and builds a resumable SQLite
reverse index for ledgers, transactions, source wallets, and direct Soroban
contract interactions. The original `index` command retains the broad fact
set; the `compact-index` command is the demo-oriented path described below.
Detailed cold call/event facts remain a separate normalization layer.

Telegram pairing and alert-policy state live in the separate writable
[`alert-control-rs`](alert-control-rs/) service. It never opens the ledger
index or serving snapshot. Its default billing mode is deliberately
unconfigured: Telegram can be linked, but no payment is collected and no alert
entitlement is invented. An explicit, bounded pilot mode exists for demos.

Run it against a local or mounted archive with:

```bash
cargo run --release --manifest-path reader-rs/Cargo.toml -- index \
  --root /srv/stargazer/raw/galexie/pubnet \
  --db ./data/stargazer-mainnet.sqlite --limit 10 \
  --decode-workers 2 --decode-ahead 4
```

Build the statically linked x86_64 Linux reader from macOS with:

```bash
./scripts/build-reader-linux-x86_64.sh
```

On the NAS, `scripts/index-one-year-nas.sh` runs the original broad index over
the latest 365-day window. It is retained for comparison, but its write
amplification is too high for the demo backfill.

## Compact demo index

`compact-index` is the preferred backfill for the product demo. It still
checkpoints every ledger and validates adjacent ledger hashes, but only writes
transactions with contract evidence and the graph and rollups needed by the
demo.

Run a bounded local sample with:

```bash
cargo run --release --manifest-path reader-rs/Cargo.toml -- compact-index \
  --root ../benchmarks/archive-soak \
  --db ./data/stargazer-demo-compact.sqlite \
  --protocol-roots data/stellar-mainnet-defi-roots.json \
  --limit 1000 --decode-workers 2 --decode-ahead 4 \
  --batch-ledgers 2000
```

The compact database is a separate schema. Do not point `compact-index` at a
database created by `index`; the reader deliberately rejects that mix. A
rerun resumes from the contiguous range in `compact_ledgers`, skips committed
ledgers, and refuses to create a checkpoint gap. Schema v2 is append-only: it
accepts only the ledger immediately after the committed high-water mark and
rejects attempts to prepend older history. Start a fresh database at the
earliest desired partition, then feed partitions oldest first. Existing compact
databases with another schema version are rejected rather than migrated in
place.

### Retention semantics

A transaction is retained when it has at least one of these facts:

- a direct or authorized contract invocation;
- a contract or diagnostic event touch;
- a contract-instance executable observation, including creation, update,
  restoration, or removal;
- a contract Wasm code observation.

Ordinary payments, offers, and other transactions with none of those facts are
not stored. Failed transactions can remain when they contain retained contract
evidence; `contract_transactions.successful` distinguishes them.
Executable and Wasm history is only materialized from successful transactions.

Wallet identities are normalized to their base `G...` account in the compact
schema. `transaction_wallets.role_mask` uses `1` for transaction source, `2`
for operation source, `4` for fee source, and `8` for a Soroban-authorized
account. That table is a transaction-level participant list; it does not imply
that every listed wallet interacted with every contract in the transaction.

Exact wallet-to-contract evidence is written to
`transaction_wallet_contracts`. A direct call is linked only to that
operation's effective source: its explicit operation source, or the transaction
source when no operation source is present. For a Soroban authorization entry,
the credential account is linked only to contract calls in that authorization
subtree. `SourceAccount` credentials use the same effective operation source.
These rules avoid the former transaction-wide wallet-by-contract Cartesian
pairing.

`wallet_contracts` is the deduplicated summary of those exact edges. Its
`role_mask` preserves whether the evidence came from transaction source (`1`),
operation source (`2`), or credential-account authorization (`8`), and
`first_ledger` records the earliest appended observation. Fee-only wallets are
never promoted into this summary. Event-only, diagnostic-only, and
version-only contract touches remain valid transaction-level evidence in
`transaction_contracts`, but they do not create an inferred wallet edge.

`transaction_contracts.interaction_mask` uses `1` for a direct call, `2` for
an authorized call, `4` for an event touch, `8` for a diagnostic
touch, and `16` for a contract-instance executable observation. A Wasm upload
may therefore be retained even when it has no contract edge yet.

### SQLite outputs

The compact schema is grouped as follows:

- checkpoint and provenance: `compact_meta`, `compact_ledgers`;
- retained transaction graph: `contract_transactions`,
  `transaction_wallets`, `transaction_contracts`,
  `transaction_wallet_contracts`, `wallet_contracts`;
- contract aggregates: `contract_usage`;
- source-backed protocol labels and aggregates: `protocols`,
  `protocol_contracts`, `transaction_protocols`, `protocol_usage`;
- executable and code history: `contract_executable_history`,
  `contract_current_executable`, `contract_availability_events`,
  `contract_code`.

Only the comparatively small executable-history tables receive secondary
indexes during ingestion:
`contract_executable_history_by_contract` orders versions by contract and
ledger, and `contract_availability_by_contract` does the same for lifecycle
events. Large secondary indexes for transaction-hash lookup and detailed
wallet history are intentionally deferred until post-backfill finalization.
This keeps random index maintenance out of the bulk write path. Before that
finalization, `wallet_contracts` supports the compact wallet-to-contract
summary efficiently, while transaction-hash and detailed per-wallet
transaction lookups may scan large tables.

`tx_hash` means the canonical 32-byte Stellar transaction hash. It is not a
transaction signature: a Stellar envelope can contain several decorated
signatures, while RPC `getTransaction` is keyed by the transaction hash. The
compact index intentionally does not store those signatures. Provider RPC
retention may not include an old backfill transaction, so
`compact_ledgers.archive_path` plus ledger sequence, apply order, and `tx_hash`
remain the durable route back to the local Galexie evidence.

### Protocol attribution

`--protocol-roots` loads `roots[]` records containing `protocol`, `role`,
`contract_id`, and optional `source_url`. A retained transaction is attributed
to a protocol when one of its contract IDs exactly matches a seeded root; a
protocol is counted at most once per transaction.

This is source-backed attribution, not chain truth. The current compact pass
does not discover factory children, infer unlabeled contracts, or assign
valid-from/valid-to dates to labels. Updating the seed upserts known rows but
does not remove stale mappings from an existing database. Use a fresh compact
database when testing a materially different registry until time-versioned
attribution is implemented.

### Contract executable history

On Stellar, a contract version is the executable referenced by its contract
instance. Wasm code is uploaded separately and deduplicated by its 32-byte
hash; uploading code alone does not upgrade a contract. `stellar_asset`
executables have no Wasm hash.

The compact state machine processes ledgers in order and currently reads
Soroban ledger-entry changes from `TransactionMeta` V3 and V4:

1. The first `State` value for an unknown contract establishes a baseline. It
   is stored once with `source_kind='observed_state'`,
   `active_before_window=1`, and a non-exact current state.
2. `Created` or `Updated` records a transition only when the executable kind or
   Wasm hash differs from the current value. Instance-storage-only updates do
   not invent new versions.
3. Every `Restored` change records a `restored` availability event. Restoring
   the same executable records no new version. A different restored value is
   retained as pre-existing state with `active_before_window=1`, not claimed as
   an upgrade at the restoration ledger.
4. Removal records a `removed` availability event; it does not erase
   executable history.
5. Contract code is stored once by Wasm hash in `contract_code`. The bytecode
   itself is not duplicated in SQLite; the archive remains the evidence store.

`contract_code` calls its provenance fields `first_seen_tx_hash` and
`first_seen_source_kind`. The source kind distinguishes `created`, `updated`,
`restored`, and `observed_state`; only a first observation with
`first_seen_source_kind='created'` should be presented as the upload
transaction. A bounded backfill that first sees code through state or
restoration metadata records that observation honestly rather than inventing
an original upload.

The transaction hash attached to a history row is the transaction in whose
metadata the state was observed or changed. The current implementation uses
the contract-instance ledger diff as its canonical signal; corroboration with
the `executable_update` system event is a later normalization step.

A one-year ledger window cannot recover the true activation ledger for an
executable that was already active at the cutoff. That is what
`active_before_window=1` means: the executable was observed in the window, but
its start time is unknown and the row's ledger must not be presented as its
deployment date. A contract that never exposes a relevant state change during
the window may also have no executable row. Removing this limitation requires
a trusted contract-state snapshot at the cutoff or replay from earlier
history.

### NAS paths and service

The deployed one-year compact run uses:

- reader: `/srv/stargazer/bin/stargazer-ledger-reader`;
- runner: `/srv/stargazer/bin/index-one-year-compact-nas.sh`;
- archive: `/srv/stargazer/raw/galexie/pubnet`;
- database: `/srv/stargazer/index/stargazer-one-year-compact.sqlite`;
- protocol seed: `/srv/stargazer/config/stellar-mainnet-defi-roots.json`;
- lock: `/srv/stargazer/index/stargazer-one-year-compact.sqlite.indexer.lock`.

The runner selects the latest approximate 365-day range, processes closed
partitions oldest first, excludes the changing live partition, and resumes the
same database after failure. Its environment overrides are
`STARGAZER_READER_BIN`, `STARGAZER_ARCHIVE_ROOT`, `STARGAZER_INDEX_DB`,
`STARGAZER_PROTOCOL_ROOTS`, `STELLAR_RETENTION_DAYS`,
`STARGAZER_DECODE_WORKERS`, `STARGAZER_DECODE_AHEAD`, and
`STARGAZER_BATCH_LEDGERS`.

The current process runs as the transient user unit
`stargazer-one-year-compact-indexer.service`; systemd exposes its generated
unit at
`/run/user/$(id -u)/systemd/transient/stargazer-one-year-compact-indexer.service`.
It restarts on failure after 30 seconds, but because it is transient it must be
launched again after a NAS reboot. From an SSH session, inspect it with:

```bash
export XDG_RUNTIME_DIR=/run/user/$(id -u)
export DBUS_SESSION_BUS_ADDRESS=unix:path=$XDG_RUNTIME_DIR/bus
systemctl --user status stargazer-one-year-compact-indexer.service
journalctl --user -u stargazer-one-year-compact-indexer.service -f
```

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

The read API currently exposes one versioned YieldBlox historical-correlation rule. It enables itself only when the bounded checkpoint window, the exact setup and USDC-drain transaction rows, and the borrower/pool/oracle edges are present. The principal borrower receives a critical confirmed-participant finding. Other wallets whose first exact relationship to the affected pool was observed by the incident end receive only an informational relationship finding, explicitly noting that the retained edge can come from a failed attempt and proves no position, balance, loss, or exploit participation. The rule never creates delivery records and does not claim generalized oracle-manipulation detection. That requires the P2 SDEX, Reflector, and Blend payload/state decoders in the parser plan.

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
