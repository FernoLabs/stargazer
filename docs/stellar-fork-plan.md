# Stellar fork plan

Status: initial architecture decision, 2026-07-18

## Working name

Use **Ferno Pulsar** as the working name for the Stellar fork.

Why it fits:

- a pulsar is a continuously observed stellar signal;
- the name describes both live indexing and security alerts;
- it keeps the Ferno parent brand without reusing the Solana-specific FireWatch identity;
- an initial web and GitHub search found no obvious blockchain product using the exact name.

This is a product codename, not trademark clearance. Check trademarks, domains, social handles, and the final repository name before launch.

Tagline: **Every Stellar contract change, turned into a security signal.**

## What the current project gives us

FireWatch already has a useful product shell:

- a durable event model with deterministic delivery retries;
- SQLite persistence and blob storage;
- program/version history and comparison views;
- protocol attribution, severity classification, and alert rules;
- Telegram subscriptions and daily summaries;
- an Axum API and SvelteKit dashboard;
- deployment, backup, and admin workflows.

The Solana ingestion path is not portable. `backend/src/solana.rs`, the Anchor IDL catalog, BPF loader parsing, Squads parsing, SNS, Solana Pay, Solana explorer links, and most curated protocol decoders must be replaced or rewritten.

The existing database and UI also use Solana words (`slot`, `signature`, `program`, `IDL`) throughout. The fork should make these concepts chain-neutral before adding Stellar logic, otherwise every Stellar feature will carry misleading names.

## Vocabulary map

| FireWatch / Solana | Ferno Pulsar / Stellar |
| --- | --- |
| slot | ledger sequence |
| transaction signature | transaction hash |
| program ID | contract ID (`C...`) |
| BPF program bytes | Wasm contract code |
| program data account | contract instance ledger entry |
| Anchor IDL | contract spec (`contractspecv0`) |
| program metadata / IDL | Wasm contract meta (`contractmetav0`) and environment meta |
| upgrade authority | contract-defined authorization; there is no universal authority field |
| Squads multisig | classic account signers/thresholds, plus contract-account implementations |
| Yellowstone/Geyser | `LedgerCloseMeta` from RPC, Captive Core, or a Galexie data lake |

Stellar documentation calls these objects **smart contracts**, not programs. Public UI and API names should use `contract` from the start.

## Proposed architecture

Keep the Rust API, rules, database ownership, delivery workers, and frontend. Add a small Go ingestion service based on the official Stellar Ingest SDK.

```text
                   historical replay
 AWS public Galexie / own data lake ------------------+
                                                       |
                                                       v
 Stellar RPC getLedgers -----> Go ledger normalizer -----> Rust ingest endpoint
          recent/live                 |                         |
                                      |                         +--> SQLite
 Captive Core -------------> optional production live source   +--> blobs/Wasm
                                                                +--> rules
                                                                +--> Telegram/API
```

Why a Go boundary:

- the official Ingest SDK already normalizes all supported sources to `xdr.LedgerCloseMeta`;
- it provides transaction and ledger-change readers for historical and live ledgers;
- the mature FireWatch product and delivery logic can remain in Rust;
- the boundary can be tested with captured ledger fixtures and replaced later if a Rust-native ingester becomes preferable.

The Go service must not write SQLite directly. It should submit one normalized ledger envelope at a time to a private Rust endpoint or Unix socket. The Rust process commits the ledger data and its checkpoint in one database transaction. Replaying the same ledger must be idempotent.

## Chain-neutral event identity

Replace the overloaded `signature` and `instruction_path` fields with explicit coordinates:

- `chain`
- `network`
- `ledger_sequence`
- `transaction_hash`
- `transaction_index`
- `operation_index`
- `event_index` or `change_index`
- `target_type`
- `target_address`

A deterministic event ID should be derived from the network passphrase hash plus the ledger and transaction coordinates, event/change kind, and target. Do not use ingestion time in an ID.

## First data model

Create these chain-specific tables rather than bending Solana version tables around Stellar:

- `stellar_ingestion_checkpoints`: source, last committed ledger, ledger hash, close time;
- `stellar_contracts`: contract ID, creator, creation transaction, creation ledger, current executable;
- `stellar_wasm_blobs`: Wasm hash, bytes/blob reference, first-seen ledger, size;
- `stellar_contract_versions`: contract ID, Wasm hash or executable type, valid-from ledger and transaction;
- `stellar_contract_interfaces`: Wasm hash, environment meta, contract meta, contract spec, parse status;
- `stellar_contract_events`: raw event XDR plus normalized topics/data;
- `stellar_account_security`: account ID, master weight, thresholds, signers, effective ledger;
- `stellar_protocol_contracts`: protocol-to-contract labels, role, source, confidence, validity range;
- `security_findings`: normalized alert candidates produced by generic and protocol-specific rules.

Store immutable data once per Wasm hash. Many contract instances can reference the same Wasm entry.

## Delivery phases

### Phase 0 — fork boundary

- create the new repository and adopt the Ferno Pulsar working name;
- introduce chain-neutral event coordinates and terminology;
- preserve delivery, entitlement, API, and dashboard behavior;
- remove Solana secrets, payment defaults, protocol snapshots, and brand assets from the fork.

Exit criterion: the app runs without a Solana RPC or Yellowstone endpoint and can accept a fixture event through the new ingestion boundary.

### Phase 1 — ledger ingestion

- implement a Go normalizer using the Stellar Ingest SDK;
- use `RPCLedgerBackend`/`getLedgers` for development and recent gap recovery;
- consume the public Galexie data lake for historical replay;
- optionally move live production ingestion to Captive Core;
- persist and verify the previous/current ledger hash chain;
- expose lag, oldest recoverable ledger, source, and last committed ledger in health status.

Exit criterion: a process restart or duplicate ledger produces no duplicate rows, and a deliberate ledger gap is recovered automatically.

### Phase 2 — index every contract and Wasm

- detect Wasm uploads and store each unique code hash and blob;
- detect contract instance creation and its initial executable;
- maintain the current instance-to-Wasm relationship;
- extract `contractenvmetav0`, `contractmetav0`, and `contractspecv0` from every Wasm;
- record parse failures and unsupported environment versions without stopping ingestion;
- distinguish Wasm contracts from built-in Stellar Asset Contracts.

Exit criterion: every observed contract instance resolves to an executable type and, for Wasm contracts, a content-addressed code record with parsed interface metadata when available.

### Phase 3 — upgrades and administration

- alert on the system `executable_update` event;
- independently confirm upgrades from the contract-instance ledger-entry diff;
- link old and new Wasm hashes and calculate spec/meta/code changes;
- identify authorization addresses from the successful invocation auth tree;
- add curated storage decoders for protocol admin, guardian, timelock, and emergency-role state.

Important limitation: Stellar does not expose a universal contract upgrade authority. Upgrade authorization is implemented by each contract. The indexer can always detect that an upgrade happened, but it cannot generically prove who is allowed to upgrade a contract in the future. That requires contract-specific analysis or verified source.

Exit criterion: every executable change has a high-priority event containing old/new code hashes, transaction actor/auth context, metadata diff, and protocol attribution when known.

### Phase 4 — multisig and governance

Treat two security models separately:

1. **Classic account multisig (`G...`)**
   - snapshot master weight, low/medium/high thresholds, and signer entries;
   - detect `SetOptions` changes and confirm them from account-entry diffs;
   - alert on signer additions/removals, lower thresholds, higher signer power, master-key changes, pre-authorized transaction signers, and hash(x) signers;
   - correlate an account with contract admin roles when protocol attribution establishes the relationship.

2. **Contract accounts / smart wallets (`C...`)**
   - index the contract and its events like any other contract;
   - classify it as multisig only from a verified registry, source/spec evidence, or a curated decoder;
   - never assume a contract account is a multisig merely because it authorizes another contract call.

Exit criterion: changes that weaken a linked admin account's authorization policy are security alerts, and smart-wallet claims include an evidence source and confidence.

### Phase 5 — DeFi activity and security rules

- attribute calls, subcalls, emitted events, token transfers, and state changes to protocol surfaces;
- start with generic rules: contract upgrades, admin/guardian changes, large token outflows, paused/unpaused state, oracle changes, dormant contract wakeups, and unusual failure bursts;
- add typed decoders from the on-chain contract spec;
- add deep protocol rules only after generic coverage is reliable;
- retain raw XDR so parsers can be improved and historical ledgers reprocessed without reacquiring data.

## Initial milestone

The smallest useful vertical slice is:

1. ingest recent mainnet ledgers from RPC;
2. discover contract creation and Wasm uploads;
3. parse contract spec/meta;
4. detect an `executable_update` system event;
5. display and send one upgrade alert;
6. replay the same range and prove idempotency.

This validates the full product path before attempting a complete mainnet backfill.

## References

- [Stellar Ingest SDK](https://developers.stellar.org/docs/data/indexers/build-your-own/ingest-sdk)
- [Ledger backends and `LedgerCloseMeta`](https://developers.stellar.org/docs/data/indexers/build-your-own/ingest-sdk/developer_guide/ledgerbackends)
- [Contract information and Wasm custom sections](https://developers.stellar.org/docs/learn/encyclopedia/contract-development/stellar-contract)
- [Contract upgrades and the `executable_update` system event](https://developers.stellar.org/docs/build/guides/conventions/upgrading-contracts)
- [Stellar signatures and classic account multisig](https://developers.stellar.org/docs/learn/fundamentals/transactions/signatures-multisig)


