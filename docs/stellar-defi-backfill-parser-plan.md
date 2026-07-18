# Stellar DeFi backfill parser plan

Status: POC-ready design, 2026-07-18

## Decision

Yes, transaction-executed historical incidents are feasible from `LedgerCloseMeta` plus a reconstructed starting state. The important boundaries are that no external registry is complete, no universal Soroban admin schema exists, and some evidence is outside `LedgerCloseMeta`: hot-archive integrity needs History Archive/Core replay or Bucket List evidence, network incidents need node telemetry, and off-chain reference prices need external data.

Build the POC as four layers:

1. a chain-complete generic ledger parser;
2. a source-attributed contract registry seeded by protocol roots and expanded from factories;
3. generic classic-account security and Soroban authorization analysis;
4. Wasm-versioned protocol decoders and cross-ledger incident rules.

The first end-to-end fixture should replay the 2026 YieldBlox incident. It exercises classic SDEX offers/trades, a Reflector price update, Blend collateral and borrow events, Soroban authorization, token movements, and a multi-ledger attack sequence in one case.

## Registry boundary

Load [`ingester/data/stellar-mainnet-defi-roots.json`](../ingester/data/stellar-mainnet-defi-roots.json) as evidence, not truth. Each record keeps the publisher and observation time. During backfill:

```text
raw LedgerCloseMeta
  -> all contract/Wasm/account facts
  -> root attribution candidates
  -> factory/router child expansion
  -> protocol decoder selected by role + Wasm hash
  -> historical findings (delivery disabled)
```

The registry needs validity ranges. A contract can migrate, be deprecated, change role, or keep the same ID while its Wasm changes. Never overwrite a former mapping.

## Live administration snapshot

The table below describes effective control observed through Mainnet RPC near ledger 63,535,4xx on 2026-07-18 and through the corresponding Horizon account state. It can change; it belongs in a time-versioned table, not a static product claim.

| Protocol | Effective control observed | Classification | Security-relevant behavior |
| --- | --- | --- | --- |
| Blend v2 | core contracts are deliberately immutable; every pool chooses its own `G...` or `C...` admin | per-pool, not protocol-wide | two-step pool admin transfer; reserve changes are queued for one week after a pool leaves Setup but unlock immediately during Setup; `update_pool` and admin `set_status` are immediate; the YieldBlox pool admin is the Soroban Governor `CANSYF...A6FQ` |
| Soroswap | factory `fee_to_setter` is `GAYPUM...25JI`, whose account has one weight-1 master signer and zero thresholds | effective single key | controls fee recipient, setter transfer and fee enablement; no upgrade or mandatory delay in core factory/router |
| Phoenix | factory admin is [`GCNPDM...QRDB`](https://horizon.stellar.org/accounts/GCNPDMUMRXBFHMM7KSXU3KVYNM73EDHBFMDK4O33HPY7DA2LXZOZQRDB), threshold 20 with signer weights 20, 10 and 10 | weighted multisig with a configured single-signer authorization path | one weight-20 signer can act alone, or the two weight-10 signers can combine; upgrades/config changes are immediate; admin-transfer `time_limit` is an expiry, not a timelock |
| Aquarius | router Admin is [`GAV5FB...J6VS`](https://horizon.stellar.org/accounts/GAV5FBMKD2ZF4X2MGWDNQYUP7KFL7MRM6HZBY7HKQLB4BRHSCCX5J6VS), a strict 2-of-3 key policy; pause, operations, rewards and emergency roles are separate accounts; the observed EmergencyAdmin is a single-key `G...` account | classic multisig plus role-based access control | upgrades wait three days normally; only Admin and EmergencyAdmin ownership transfers have the three-day role-transfer delay, while the other role replacements are immediate Admin actions; EmergencyAdmin can enable emergency mode, but the 2-of-3 Admin must still apply the upgrade |
| DeFindex | factory admin is [`GAJZKZ...L35P`](https://horizon.stellar.org/accounts/GAJZKZVWJ237N23KAGBRIFNVE7KOUF27OTIKQPXF3TIJEGYM776WL35P), a one-signer account; each vault has Manager, EmergencyManager, FeeReceiver and RebalanceManager addresses | factory single key plus per-vault custom roles | factory changes affect future vaults; an upgradable vault's Manager can upgrade immediately; Manager/EmergencyManager can pause or rescue strategies |
| Reflector | the USD oracle admin [`GCJ2RD...MRFB`](https://horizon.stellar.org/accounts/GCJ2RDQLOVNOYBD2LELEGEVIDZ3LRLPHGANVOFJZXTUQHKHVZX2LMRFB) and Stellar-assets oracle admin [`GC2M2U...6RFA`](https://horizon.stellar.org/accounts/GC2M2UTZ57GSENZBTPMLPB6QSAL2USIPHMXSUCT2I3V24GYWF3EI6RFA) are strict 4-of-7 accounts with master weight zero | strict classic multisig | oracle updates/config/upgrades are multisig-authorized but have no Wasm-upgrade timelock; the separate DAO records ballots and a multisig-authenticated collective decision rather than seven individual on-chain votes |
| FxDAO | the documented vault Admin, ProtocolManager and locking-pool Manager currently resolve to [`GC7JVO...NIKV`](https://horizon.stellar.org/accounts/GC7JVOXZJSHY3GHKWUJWKIUYWEJ4RZABSRQZQ5JBZEC5QUTYBUHVNIKV), a one-signer account | effective single key with distinct contract roles | vault `upgrade` is Admin-only, `panic_mode` and oracle changes are ProtocolManager-only, and locking-pool upgrade is Manager-only; preserve those roles even while they resolve to the same account |

“Multisig” is not a sufficient output field. Store at least:

```text
policy kind: single-key | weighted-with-single-key-path | strict-m-of-n | contract-auth | unknown
master weight
low / medium / high thresholds
signer key, type, and weight
minimum satisfying signer sets
operator independence: externally-attested | unknown
admin/role address
valid from/to ledger
evidence transaction, operation, and ledger
```

For a `G...` authorizer, reconstruct the applicable pre-invocation AccountEntry. Transaction signature count is not proof of the policy: a threshold-20 account with one weight-20 signer and two weight-10 signers has both one-key and two-key authorization paths. “Strict M-of-N” proves distinct on-chain signer keys and required weights, not independent people, devices or organizations; operator independence remains unknown unless externally attested. For a `C...` authorizer, combine transaction auth entries with call-stack/invocation evidence and time-versioned role state. A contract can satisfy `require_auth` as the immediate invoker or through `authorize_as_current_contract`, so there may be no standalone address-credential `SorobanAuthorizationEntry` for that admin. Use a curated/source-verified decoder to understand the policy.

## Confirmed exploit fixture: YieldBlox

### What happened

The YieldBlox incident occurred on 2026-02-22 UTC; the attacker placed the setup offer at 23:38:51 UTC on February 21. The attacker manipulated the very shallow USTRY/USDC classic SDEX market. A trade of only about 5.35 USDC cleared USTRY near 106.74 USDC. Reflector then published the market-derived value. The isolated YieldBlox Blend v2 pool accepted USTRY as collateral without adequate liquidity/deviation/fallback controls, after which the attacker borrowed approximately 1,000,196.7040837 USDC and 61,249,278.3064502 XLM.

This was a YieldBlox pool/oracle configuration failure, not a Blend v2 core-contract exploit. There is no evidence of an unauthorized Reflector update or Reflector contract exploit; Reflector propagated the manipulated configured market input. That distinction must be represented in finding attribution.

Relevant identities:

| Role | Address |
| --- | --- |
| YieldBlox Blend v2 pool | `CCCCIQSDILITHMM7PBSLVDT5MISSY7R26MNZXCX4H7J5JQ5FPIYOGYFS` |
| Reflector Stellar-assets oracle | `CALI2BYU2JE6WVRUFYTS6MSBNEHGJ35P4AVCZYF3B6QOE3QKOB2PLE6M` |
| YieldBlox Governor / pool admin | `CANSYFVMIP7JVYEZQ463Y2I2VLEVNLDJJ4QNZTDBGLOOGKURPTW4A6FQ` |
| principal borrower | `GBO7VUL2TOKPWFAWKATIW7K3QYA7WQ63VDY5CAE6AFUUX6BHZBOC2WXC` |

### Ground-truth sequence

| Ledger | UTC | Transaction | Expected fact |
| ---: | --- | --- | --- |
| 61,339,940 | 2026-02-21 23:38:51 | [`09e1a9…c7cdb`](https://horizon.stellar.org/transactions/09e1a9d1197c9bf0af4e87da328c4f2d5eb49b487630aa61991fb5c1c4637cdb) | abnormal USTRY sell offer near 106.74 USDC |
| 61,340,263 | 2026-02-22 00:10:21 | [`60fe03…5bef1`](https://horizon.stellar.org/transactions/60fe039e96e88402d175c8de68e80651874ab125880dd384a1636914ba95bef1) | about 0.0501003 USTRY crosses for about 5.3475699 USDC |
| 61,340,315 | 2026-02-22 00:15:22 | [`56466a…2be51`](https://horizon.stellar.org/transactions/56466ad66cd5c49a251b9f1c7c0cf6b1b2f62d121b58ff856e2b43673b22be51) | Reflector `set_price`; earliest high-confidence cross-protocol alert point |
| 61,340,381 | 2026-02-22 00:21:51 | [`b810ba…a597`](https://horizon.stellar.org/transactions/b810ba4a8d7547bb024f17b58c32cc644533176206d0f8178d4ba0a545cb7597) | about 13,003.2374199 USTRY enters the pool |
| 61,340,384 | 2026-02-22 00:22:09 | [`ae721c…a31ff`](https://horizon.stellar.org/transactions/ae721cacee382bdecac8d2c47286ecd42cb4711f658bb2aec7cba60dc64a31ff) | about 1,000,196.7040837 USDC leaves the pool |
| 61,340,405 | 2026-02-22 00:24:10 | [`81f304…c85a6`](https://horizon.stellar.org/transactions/81f304ae627ba294df0f9c0708a58a2fe770b39d65f8cd0efe1d7d6d6cc885a6) | another 140,000 USTRY enters the pool |
| 61,340,408 | 2026-02-22 00:24:27 | [`3e81a3…d4657`](https://horizon.stellar.org/transactions/3e81a3f7b6e17cc22d0a1f33e9dcf90e5664b125b9e61f108b8d2f082f2d4657) | about 61,249,278.3064502 XLM leaves the pool |

### Detection rules

Emit composable facts first; correlate them into findings afterward.

**Classic SDEX facts**

- decoded `ManageSellOffer`, `ManageBuyOffer`, path-payment claims, offers created/updated/deleted, and claimed-offer fills;
- normalized base/quote price, token decimals, notional, trailing median/TWAP deviation, recent volume, order-book depth, and trader concentration;
- order-book depth reconstructed from active `OfferEntry` state; it is not emitted by the incident transaction, so a bounded fixture needs a trusted pre-range snapshot;
- new or amended offer placed far from the prior market;
- extreme price move produced by negligible notional and shallow depth.

**Reflector facts**

- contract/feed/asset, previous and new price, decimals, timestamp, freshness, resolution, and source-market identity;
- underlying trade window volume/depth and concentration where the feed is based on Stellar assets;
- deviation from the feed's history and other archive-derived on-chain feeds; optional off-chain reference feeds must be stored as external enrichment with provider and observation-time provenance;
- authorized admin account and effective signer policy at that ledger.

**Blend facts**

- pool, admin, oracle, reserves, reserve configuration, collateral/liability factors, status and queued changes;
- decoded supply, collateral supply, borrow, repay, liquidation and auction events;
- collateral value at the configured price and archive-derived on-chain reference prices; an off-chain robust price is optional external enrichment, not a deterministic replay fact;
- borrow as a percentage of pool cash, utilization jump, user health factor, and reserve outflow.

**Cross-ledger finding**

```text
thin-market price spike
  -> oracle publishes the spike
  -> same asset enters a lending pool as collateral
  -> USDC/XLM reserve outflow approaches pool cash
  -> recipient routes assets onward shortly afterward
```

Score this sequence by price deviation, market depth, oracle-to-borrow ledger distance, borrow/reserve ratio, collateral concentration, and destination novelty. Bridge/exchange routing is corroboration, not proof.

False-positive controls:

- normalize decimals and explicit redenominations;
- require both extreme deviation and low liquidity/volume for the market-manipulation rule;
- compare multiple provenance-tagged reference prices and allow known depegs/corporate actions to be annotated;
- distinguish an oracle faithfully reporting a bad market from an unauthorized oracle update;
- require a lending/asset-outflow consequence before promoting the combined finding to critical;
- bind rules to protocol role and Wasm hash so old audit findings do not fire against fixed code.

## Other historical security cases

Keep these categories separate in the database and UI.

| Category | Case | What a backfill can prove |
| --- | --- | --- |
| confirmed DeFi exploit | YieldBlox, 2026-02-22 | complete SDEX → oracle → collateral → borrow/outflow sequence |
| network state-integrity incident, no theft | Protocol 23 archival corruption, 2025-09-04 to 2025-10-23 | `LedgerCloseMeta` exposes evicted keys and values when they are later restored, so those restored entries can be compared; it does not expose the archived value or the hot-archive amendment, and never-restored affected entries require deterministic Core replay plus Bucket List/hot-archive evidence |
| availability incident, no loss | 2019 network halt | contiguous ledger sequences with valid hash chaining but an approximately 67-minute close-time gap; distinguish from a local missing archive |
| disclosed, not exploited on Mainnet | 2024 fee-bump refund bug | negative-control rule: fee-bump Soroban refunds must credit the sponsor; fixed before Protocol 20 activation |
| disclosed, no reported exploitation | 2025 memo/muxed-source auth mutability | find equivalent address-auth invocations with altered memo/muxed source during the vulnerable protocol range |
| disclosed, no known exploit | 2026 missing signer address in Soroban signature payload | look for a reused signer/signature across credential-owner addresses, then require matching nonce/expiry, threshold, unsafe admin rotation or `require_auth_for_args`, and a successful state-changing call |
| audit finding, not exploit | Blend v2 Code4rena findings | run supply/accounting/utilization invariants only against affected pre-fix Wasm hashes |
| audit finding, not exploit | Soroswap unbounded pair-vector factory DoS | track `create_pair` bursts and instance-entry size only for the vulnerable Wasm version |

Do not label wallet, DNS, exchange, or private-key compromises as smart-contract exploits merely because their transfers are visible on Stellar. Ledger data proves the transaction, not how a key was compromised.

## Parser architecture

### Stage 1: chain facts

For every ledger, transaction, operation and sub-invocation:

- preserve raw XDR or its content-addressed location;
- decode successful and failed transactions, but apply state only for successful execution;
- capture operation ordering, source account, memo, fee-bump sponsor, signatures, Soroban auth entries and invocation tree;
- normalize contract events, diagnostic events, classic/SAC token movements, and every ledger-entry change;
- materialize AccountEntry, ContractData, ContractCode, TTL and contract-instance histories;
- detect contract creation, Wasm upload, evicted keys, later-restored values, and `executable_update` plus executable diffs; do not claim that `LedgerCloseMeta` exposes archived values or hot-archive amendments.

### Stage 2: source-attributed registry

Suggested tables:

```text
registry_sources(id, kind, url, fetched_at, response_hash, license, status)
contract_candidates(network, contract_id, protocol, role, source_id, confidence, observed_at)
contract_attributions(network, contract_id, protocol, role, valid_from, valid_to, evidence_id)
contract_versions(contract_id, wasm_hash, valid_from, valid_to, source_commit, attestation)
factory_relations(factory_id, child_id, child_role, creation_ledger, creation_tx, verified)
```

Promotion checks include address/network validity, on-chain existence, factory relationship, current/historical Wasm, creator, source attestation, manifest conflicts and role-specific spec fingerprint.

### Stage 3: authorization and governance

```text
account_security_snapshots(account_id, valid_from, valid_to, master_weight,
                           low_threshold, medium_threshold, high_threshold)
account_signers(account_id, signer_key, signer_type, weight, valid_from, valid_to)
contract_roles(contract_id, role, authorizer, valid_from, valid_to, evidence)
governance_actions(governor_id, proposal_id, state, targets, functions, args,
                   created_ledger, executable_ledger, executed_tx)
```

Process `SetOptions` intent and the resulting AccountEntry diff. Evaluate account auth at the exact operation/sub-invocation point. For contract authorizers, decode known Governor/custom-account proposal lifecycle and execution targets. Always show the final target/function/arguments that governance executed; a “proposal executed” label without payload decoding is not useful security telemetry.

### Stage 4: versioned protocol decoders

| Decoder | Initial hooks |
| --- | --- |
| Blend v2 | factory `deploy`; pool admin proposal/acceptance; delayed reserve queue/apply outside Setup; immediate pool config/status changes; supply/collateral/borrow/repay/liquidation/auction events; reserve and position invariants |
| Reflector | `set_price`/update event, feed metadata, admin/config/Wasm changes, DAO ballot/vote state |
| Aquarius | pool creation/removal, privileged-role changes, pause/emergency mode, `commit/apply/revert_upgrade`, swaps/deposits/withdrawals/rewards |
| DeFindex | factory create and admin/fee/Wasm-for-future-vault events; vault roles, upgrade, pause/unpause/rescue/rebalance; strategy deposits/withdrawals/harvests |
| Soroswap | `new_pair`, fee/setter/fees changes, pair deposit/swap/withdraw/sync, router swap paths |
| Phoenix | factory pool creation, config/admin changes, pool/stake flows, executable diffs for immediate upgrades even when no semantic event exists |
| FxDAO | admin/manager/oracle/panic/config calls and storage diffs, vault debt/collateral/liquidation; source has missing event coverage, so invocation plus state/executable diff is mandatory |

Decoder output must contain a `decoder_version`, recognized Wasm hash set, raw event reference, and a confidence. Unknown/new Wasm continues through the generic parser and raises a coverage finding rather than being silently decoded with an old schema.

## POC worklist

### P0 — generic replay

- [x] Capture checkpoint `LedgerCloseMeta` fixtures for ledgers 61,339,940, 61,340,263, 61,340,315, 61,340,381, 61,340,384, 61,340,405 and 61,340,408 through archive `getLedgers`; use these for exact transaction assertions, not anomaly-window statistics.
- [ ] Capture a continuous Mainnet range from 61,339,940 through at least 61,340,450 and reconstruct or capture the active-ledger starting state immediately before the range. This supplies prior offers, oracle state, pool cash and trailing price/volume context; extend the end if onward-routing correlation is enabled.
- [ ] Decode classic offers/claims, Soroban events/auth trees, token transfers and ledger-entry changes into stable envelopes.
- [ ] Add idempotent ledger/transaction/operation/event/change coordinates and parser-version fields.
- [ ] Materialize account thresholds/signers and contract executable history.

### P1 — registry expansion

- [ ] Load and validate the checked-in Mainnet root seed.
- [ ] Implement Soroswap and DeFindex manifest adapters first.
- [ ] Expand/reconcile Soroswap pairs, Aquarius pools, Phoenix pools and DeFindex vaults from their factory/router state and events.
- [ ] Add Blend event-only factory expansion and `is_pool` verification.
- [ ] Snapshot Reflector runtime config and reconcile its three oracle roots.
- [ ] Add an admin review queue for label, Wasm and role conflicts.

### P2 — YieldBlox detector

- [ ] Implement SDEX thin-market price deviation/depth facts.
- [ ] Implement Reflector feed updates and trailing/reference deviation.
- [ ] Implement Blend reserve, collateral, borrow and pool-cash facts.
- [ ] Correlate the exact ground-truth sequence above with historical delivery disabled.
- [ ] Store cause attribution as `pool_configuration`, with `Blend core affected=false` and `Reflector unauthorized=false`.

### P3 — governance and protocol coverage

- [ ] Classify effective account security, including minimum satisfying signer sets.
- [ ] Decode Aquarius delay/emergency bypass and all privileged-role changes.
- [ ] Decode Blend/DeFindex per-pool/per-vault roles instead of inventing a global admin.
- [ ] Decode Soroban Governor proposals, targets and execution for YieldBlox and other known DAOs.
- [ ] Add Phoenix/Reflector/FxDAO executable-diff upgrades where no semantic event is emitted.
- [ ] Attribute Templar and Sushi production roots before claiming broad Stellar DeFi coverage.

## POC acceptance criteria

1. Replaying any fixture twice produces no duplicate facts or findings.
2. The checkpoint ledgers reproduce the identities, assets, normalized amounts and ordering above; the continuous fixture plus starting state deterministically reconstructs trailing market depth, oracle state and pre-transaction pool cash.
3. The detector raises a high-severity oracle-risk finding at ledger 61,340,315, identifies the critical USDC reserve outflow at ledger 61,340,384, and raises the critical combined reserve-drain sequence by ledger 61,340,408 without calling Blend or Reflector exploited.
4. Current-state reconciliation can account for every child returned by a supported factory/router and reports stale API/manifest records explicitly.
5. The authorization engine classifies Reflector's key policy as strict 4-of-7, Aquarius Admin as strict 2-of-3, Phoenix as weighted with a configured single-signer path, and Soroswap/DeFindex factory/FxDAO as effective single key at the observation ledger; operator independence remains `unknown` without external attestation.
6. A new unknown Wasm hash generates a decoder-coverage finding and still preserves generic events/state changes.
7. Backfill findings are queryable but never delivered as live alerts unless an operator explicitly enables replay delivery.

## References

- [BlockSec YieldBlox analysis](https://blocksec.com/blog/yieldblox-dao-incident-on-stellar-oracle-misconfiguration-enabled-a-10m-drain)
- [Blockaid YieldBlox response](https://blockaid.io/blog/73-quarantined-how-blockaid-and-stellar-validators-contained-a-10m-price-manipulation-attack)
- [Blend v2 pool admin and configuration source](https://github.com/blend-capital/blend-contracts-v2/blob/main/pool/src/contract.rs)
- [Soroswap factory source and events](https://github.com/soroswap/core/blob/main/contracts/factory/src/lib.rs)
- [Phoenix contracts and Mainnet upgrade roots](https://github.com/Phoenix-Protocol-Group/phoenix-contracts)
- [Aquarius Soroban contracts](https://github.com/AquaToken/soroban-amm)
- [DeFindex contract architecture and roles](https://github.com/paltalabs/defindex/blob/main/docs/defindex-contracts.md)
- [Reflector oracle and multisig design](https://github.com/reflector-network/reflector-contract)
- [FxDAO contracts](https://github.com/FxDAO/FxDAO-SC)
- [Stellar state archival incident postmortem](https://stellar.org/blog/foundation-news/state-archival-issue-post-mortem)
- [CAP-76 and affected-entry data](https://github.com/stellar/stellar-protocol/blob/master/core/cap-0076.md)
- [2019 network halt report](https://stellar.org/blog/developers/may-15th-network-halt)
- [Fee-bump bug disclosure](https://stellar.org/blog/developers/fee-bump-bug-disclosure)
- [Memo/auth mutability advisory](https://github.com/stellar/stellar-core/security/advisories/GHSA-3p8h-7v82-ffvq)
- [Soroban signature-payload security notice](https://github.com/orgs/stellar/discussions/1899)
