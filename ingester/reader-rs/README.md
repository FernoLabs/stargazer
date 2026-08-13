# Stargazer archive indexer

This crate streams Stellar Galexie `*.xdr.zst` ledgers into a resumable SQLite
reverse index. It is designed to process the multi-terabyte pubnet archive with
bounded memory:

- archive discovery retains at most one 64,000-ledger partition of paths;
- zstd is decoded directly into the XDR reader, without a decompressed `Vec`;
- XDR depth, decoded allocation size, compressed size, and zstd window are capped;
- ledger decoding uses a bounded worker pool while SQLite writes remain ordered
  on one thread;
- database writes commit in configurable atomic batches and existing ledgers are skipped;
- every transaction envelope is joined to its result by the canonical network hash;
- transactions are stored in apply order, including fee-bump sources;
- muxed accounts are normalized to their base `G...` identity plus mux ID;
- adjacent indexed ledgers must agree on their hash chain;
- each database is permanently bound to one Stellar network ID.

## Build and test

```bash
cargo test --manifest-path ingester/reader-rs/Cargo.toml
cargo build --release --manifest-path ingester/reader-rs/Cargo.toml
```

The existing x86_64 Linux cross-build script builds every binary in the crate;
the API artifact is
`target/x86_64-unknown-linux-musl/release/stargazer-read-api`.

## Inspect or validate files

```bash
ingester/reader-rs/target/release/stargazer-ledger-reader inspect \
  --root /path/to/galexie/pubnet --limit 10

ingester/reader-rs/target/release/stargazer-ledger-reader validate \
  --root /path/to/galexie/pubnet --limit 10
```

`validate` performs a second bounded pass to emit a SHA-256 digest of the
decompressed XDR. `inspect` is faster and emits extracted ledger metrics.

## Build or resume an index

Keep SQLite on a local SSD while reading archive files from the NAS:

```bash
ingester/reader-rs/target/release/stargazer-ledger-reader index \
  --root /volume1/blockzilla/stargazer/raw/galexie/pubnet \
  --db ./data/stargazer-mainnet.sqlite \
  --batch-ledgers 200 \
  --decode-workers 2 \
  --decode-ahead 4
```

Re-running the command safely skips committed ledgers. A failed decode or write
rolls back the active batch. Decoders may finish out of order, but ledgers are
always written in archive order. `--decode-ahead` is a hard bound on queued,
active, and decoded ledgers; lowering it reduces peak memory. It must be at least
`--decode-workers` and defaults to twice that value.

The defaults come from a 1,000-ledger recent-history soak: two decoders and
200-ledger commits outperformed both serial indexing and four decoders while
keeping WAL growth and decoded-ledger memory bounded.

The hot schema stores ledgers, transactions, effective source accounts, fee
sources, explicit operation sources, and one deduplicated transaction/contract
edge. Role flags distinguish transaction, operation, and fee sources; contract
flags distinguish direct calls, authorized calls, events, and diagnostic events.
This keeps the full-history reverse index substantially smaller than storing
every extracted fact as a separate indexed row. A wallet-to-contract dependency
query is:

```sql
SELECT c.strkey AS contract_id,
       COUNT(DISTINCT a.transaction_hash) AS transactions,
       MIN(a.ledger_sequence) AS first_ledger,
       MAX(a.ledger_sequence) AS last_ledger
FROM accounts w
JOIN account_transactions a USING (account_id)
JOIN transaction_contracts tc USING (transaction_hash)
JOIN contracts c ON c.contract_id = tc.contract_id
WHERE w.strkey = ?1
GROUP BY c.strkey
ORDER BY transactions DESC;
```

The same query is exposed by the CLI; `M...` muxed addresses are normalized to
their underlying `G...` wallet identity:

```bash
ingester/reader-rs/target/release/stargazer-ledger-reader wallet \
  --db ./data/stargazer-mainnet.sqlite \
  --account G... --include-transactions
```

Function/depth-level call facts are deliberately kept out of the hot SQLite
schema; they can be written to a cold analytical store later. Project/protocol
labels remain the next normalization layer.

## Read-only wallet API

`stargazer-read-api` exposes the completed compact index without giving the web
process write access to it. It validates compact schema version 2 and the public
network ID at startup, opens SQLite with `READ_ONLY` and `query_only`, caps page
sizes, interrupts SQLite statements at a deadline, and limits concurrent query
workers.

On the NAS, bind explicitly to the LAN interface and allow the exact browser
origin used by the Svelte development server:

```bash
ingester/reader-rs/target/release/stargazer-read-api \
  --db /volume1/blockzilla/stargazer/index/stargazer-one-year-compact.sqlite \
  --serving-db /volume1/blockzilla/stargazer/index/stargazer-wallet-serving.sqlite \
  --host 0.0.0.0 \
  --port 8787 \
  --cors-origin http://192.168.1.45:5173
```

The equivalent environment variables are `STARGAZER_INDEX_DB`,
`STARGAZER_SERVING_DB`, `STARGAZER_API_HOST`, `STARGAZER_API_PORT`, and the
comma-delimited `STARGAZER_API_CORS_ORIGINS`. The query deadline, concurrency,
and configured page cap default to 3 seconds, four reads, and 250 rows. The
absolute wallet/leaderboard page cap is 500; the real-wallet sample cap is 20.
Place an authenticated, rate-limiting reverse proxy in front of the service
before exposing it outside a trusted network.

Endpoints:

- `GET /health` reports compact coverage, the exact retained contract-transaction
  count, and dynamically detected capabilities. The JSON field
  `source.indexedContractTransactionCount` is the append-only
  `contract_transactions` high-water mark: each transaction that interacted
  with at least one contract is counted once, regardless of how many wallets,
  contracts, or protocols it touched. It does **not** count non-contract
  Stellar transactions in the covered ledgers and should be labelled
  "contract transactions indexed", not "all transactions scanned". The
  integer-primary-key lookup is constant-memory and avoids scanning the large
  transaction table at API startup.
- `GET /api/wallets/{G-or-M-address}?limit=100&cursor=C...` returns exact
  `wallet_contracts` relationships and source-backed `protocol_contracts`
  attribution. Unmatched relationships remain in the response with
  `attributionStatus="unattributed"` and their checksum-valid C-address as the
  placeholder ID; the API never invents a protocol name. M-addresses are
  normalized to their underlying G-address.
- `GET /api/contracts/unattributed?limit=100&cursor=C...` traverses every
  contract observed in `contract_usage` that is not one of the verified roots.
  Results are ordered by raw contract ID and use keyset pagination. The
  response intentionally has no global total or activity sort because either
  would require an unbounded scan without an offline serving index.
- `GET /api/contracts?attribution=all|unattributed&limit=100&cursor=C...`
  exposes the same bounded catalog with an explicit attribution filter.
- `GET /api/examples/wallets?limit=5` returns real lexicographically sampled
  relationship rows. It is explicitly not a ranking.
- `GET /api/leaderboard?metric=transactions|protocols&limit=50&offset=0`
  reads the finalized serving snapshot described below.
- `GET /api/protocols` returns every source-backed protocol root, retained
  contract usage, current executable, chronological executable history, and an
  informational alert for each exact `updated` executable transition. A
  pre-window observation is kept as a baseline and is never mislabeled as an
  upgrade.

The global contract catalog means contracts observed inside the indexed ledger
range, not every contract ever deployed on Stellar. Per-wallet relationships
remain stricter: only exact direct-call or authorization-subtree evidence
creates a wallet edge; fee-only, event-only, diagnostic-only, and version-only
touches do not.

The compact schema has no account-first index for historical transaction
counts. Until a serving snapshot exists, the wallet response returns `null`
(not zero) for transaction totals and last activity, reports alerts as
unavailable, and the leaderboard returns HTTP 503 with error code
`leaderboard_unavailable`. Contract and protocol relationship counts remain
real compact-index facts.

The optional serving database is detected and validated on every request, so a
finalizer can atomically rename it into place without restarting the API. Its
schema contract is:

```sql
CREATE TABLE serving_meta(
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
) WITHOUT ROWID;

-- Required keys:
-- schema_version = '1'
-- source_indexed_ledger_start = compact MIN(sequence)
-- source_indexed_ledger_end = compact MAX(sequence)
-- generated_at = RFC3339 timestamp

CREATE TABLE wallet_activity(
  account_id BLOB PRIMARY KEY CHECK(length(account_id)=32),
  transaction_count INTEGER NOT NULL CHECK(transaction_count>=0),
  successful_transaction_count INTEGER NOT NULL
    CHECK(successful_transaction_count>=0
      AND successful_transaction_count<=transaction_count),
  protocol_count INTEGER NOT NULL CHECK(protocol_count>=0),
  contract_count INTEGER NOT NULL CHECK(contract_count>=0),
  first_ledger INTEGER NOT NULL,
  last_ledger INTEGER NOT NULL
) WITHOUT ROWID;

CREATE INDEX wallet_activity_by_transactions
  ON wallet_activity(transaction_count DESC, account_id ASC);
CREATE INDEX wallet_activity_by_protocols
  ON wallet_activity(
    protocol_count DESC,
    transaction_count DESC,
    account_id ASC
  );
```

The API refuses counts and rankings if the metadata range differs from compact
coverage or either named index has a different key order. Per-wallet alert
findings and delivery attempts are not part of either SQLite database yet and
are always described as unavailable rather than fabricated. The protocol
catalog's executable-update alerts are instead derived deterministically from
exact archive metadata and do not claim that a notification was delivered.

## Benchmark

```bash
ingester/reader-rs/target/release/stargazer-ledger-reader benchmark \
  --root benchmarks/archive-samples --limit 4 --iterations 25
```

The local fixture manifest is in
`benchmarks/archive-samples/manifest.json`; compressed fixtures remain ignored by
Git and can be restored from the NAS.

The larger ignored soak in `benchmarks/archive-soak` covers ledgers
63,615,000–63,615,999 (404,555 transactions). On the 2026-07-24 local run, the
compact schema produced a 176 MiB database instead of 480 MiB for the earlier
fact-per-row schema. With 200-ledger commits, throughput was 24.7 ledgers/s with
one decoder, 56.6 with two, and 50.1 with four. Two workers is therefore the
current default; the NAS production host still needs its own full-partition
benchmark.
