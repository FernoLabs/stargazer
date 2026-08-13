# Wallet activity finalizer

`stargazer-wallet-finalizer` turns the completed compact archive index into the small serving
database used for wallet summaries and both leaderboard orderings. It reads the compact database
strictly read-only and publishes a separate SQLite file only after every phase succeeds.

## Why it is a separate pass

`transaction_wallet_contracts` is ordered by transaction, which is ideal for ingestion and exact
wallet-to-contract attribution but not for ranking arbitrary wallets. Building a wallet-first index
on the 478 GiB source would be large and would modify the historical evidence store. The finalizer
instead performs bounded-memory sequential passes:

1. stream exact edges in transaction order and emit one record per `(transaction, wallet)`;
2. partition those 37-byte records into account shards with durable checkpoints;
3. aggregate one shard at a time into `wallet_activity`;
4. stream `wallet_contracts` in account order to add distinct contract and verified-root protocol
   counts;
5. create the two leaderboard indexes and atomically rename the completed serving database.

The shard files require approximately `37 × exact wallet-transactions` bytes. They are deliberately
kept after publication so a run can be audited; remove the work directory only after the published
database has been backed up and verified.

## Build for the NAS

```bash
./ingester/scripts/build-wallet-finalizer-linux-x86_64.sh
```

## Run

```bash
stargazer-wallet-finalizer \
  --compact-db /srv/stargazer/index/stargazer-one-year-compact.sqlite \
  --output /srv/stargazer/index/stargazer-serving.sqlite \
  --work-dir /srv/stargazer/tmp/wallet-finalizer
```

The default 256 shards keep peak memory bounded on the 8 GiB NAS. Progress is flushed to
`progress.json`; rerunning the exact same command resumes from the last durable transaction or
account checkpoint. Do not change the shard count for an existing work directory.

The published database contains `wallet_activity`, `wallet_activity_by_transactions`,
`wallet_activity_by_protocols`, and coverage metadata checked by the read API. Counts use only exact
wallet-to-contract edges. Fee-only participants and transaction-wide Cartesian attribution are not
included.
