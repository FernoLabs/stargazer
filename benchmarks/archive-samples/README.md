# Stellar archive benchmark corpus

The compressed fixtures in this directory are intentionally local-only. They
come from Galexie pubnet partitions on the NAS. The checked-in `manifest.json`
records their hashes and expected extraction counts so another machine can
reconstruct and verify the same corpus without committing binary archive data
to Git.

The corpus currently covers:

- three of the largest compressed ledgers in the partition;
- a high-transaction ledger (998 transactions);
- a consecutive pair for sequence and previous-ledger-hash validation;
- ledger `63,416,408`, which contains a canonical contract executable upgrade
  with both the prior and replacement Wasm hashes in ledger-entry changes.

Run the extraction benchmark with:

```bash
cargo run --release --manifest-path ingester/reader-rs/Cargo.toml -- \
  benchmark --root benchmarks/archive-samples --limit 5 --iterations 25
```

Run the durable SQLite path with:

```bash
cargo run --release --manifest-path ingester/reader-rs/Cargo.toml -- \
  index --root benchmarks/archive-samples --db /tmp/stargazer-index.sqlite
```
