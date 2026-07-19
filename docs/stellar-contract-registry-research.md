# Stellar contract registry research

Status: source assessment, 2026-07-18

## Conclusion

There is no single registry that is complete enough to define “all Stellar DeFi contracts.” Use on-chain ledger ingestion for completeness and several external sources for labels, verification evidence, protocol grouping, and prioritization.

The useful discovery stack is:

1. **On-chain `LedgerCloseMeta`** — complete contract/Wasm inventory and version history;
2. **StellarExpert contract API** — convenient recent contract inventory, activity counts, Wasm lookup, version history, and build-validation status;
3. **Stellar Lab popular-contract list** — small curated DeFi seed set;
4. **Stellar ecosystem directory and ecosystem-resources** — protocol names, categories, websites, and source repositories;
5. **DeFiLlama** — market-priority and protocol discovery, never contract identity truth;
6. **protocol-owned deployment manifests and verified source attestations** — canonical role labels and the highest-confidence attribution available outside the ledger.

The POC seed assembled from those sources is checked in at [`ingester/data/stellar-mainnet-defi-roots.json`](../ingester/data/stellar-mainnet-defi-roots.json). Its current snapshot has 36 records across seven protocols plus two incident contracts. It contains selected security-monitoring roots and published DeFindex strategy deployments rather than pretending to be a complete child-contract list. Factories, routers, protocol APIs and historical events expand those roots into pools, vaults and other children during ingestion.

## Current source audit

Observed on 2026-07-18:

| Source | What it gives us | Current breadth | Important limitation |
| --- | --- | --- | --- |
| [SDF ecosystem-resources DeFi guide](https://github.com/stellar/ecosystem-resources/blob/main/defi/README.md) | project names, categories, sites, and repositories | Blend, Soroswap, Aquarius, Phoenix, DeFindex, Orbit and stablecoins | project directory, not a deployment registry |
| [Stellar Lab popular contracts](https://github.com/stellar/laboratory/blob/main/src/constants/popularSorobanContracts.ts) | curated labels and root contract IDs | 28 hard-coded Mainnet entries; file last changed 2025-12-17 | incomplete and sometimes role-stale: Phoenix's listed address is its multihop contract, not its factory; an “Aqua Rewards” entry is actually a pool |
| [StellarExpert contract API](https://api.stellar.expert/explorer/public/contract?limit=200&order=desc) | deployed contract universe, creator, current Wasm, activity and validation evidence | pageable across all observed Mainnet contracts | contract endpoints are absent from the published OpenAPI and labels are incomplete |
| [Stellar Hubble/dbt](https://github.com/stellar/stellar-dbt-public) | normalized contract data/code, events, token transfers and signer history | generic network-wide analytical models | no maintained DeFi ownership/role registry |
| protocol-owned manifests and APIs | canonical roots, roles, Wasm hashes and child instances | best available attribution source | every protocol publishes a different shape; versions and deprecated records must be retained |

The comment in Stellar Lab that its popular list came from `stellar-dbt` should not be interpreted as a public all-DeFi registry. The public dbt repository contains generic complete ledger models, not the Lab's protocol label table.

## Protocol-owned sources

| Protocol | Canonical discovery path | Observed expansion | Parser consequence |
| --- | --- | --- | --- |
| Blend v2 | [deployment docs](https://docs.blend.capital/mainnet-deployments) and [pinned Blend Utils manifest](https://github.com/blend-capital/blend-utils/blob/b05242df30b6b6caf9d317646f754541824a5a8b/mainnet.contracts.json) | pool factory exposes `deploy` and `is_pool`, but no list-all method | replay factory `deploy` events and verify each child with `is_pool`; keep v1/v2 hashes separate |
| Soroswap | [pinned core Mainnet manifest](https://github.com/soroswap/core/blob/bb90a65556d8eee0dc698ac75de0f280e547fedc/public/mainnet.contracts.json) and [pinned aggregator manifest](https://github.com/soroswap/aggregator/blob/84de10e0f8d26168b4a76f8c23b963e50917517c/public/mainnet.contracts.json) | factory reported 202 pairs on-chain | replay `SoroswapFactory/new_pair`, then reconcile `all_pairs_length` and `all_pairs` |
| Aquarius | [Mainnet integration constants](https://docs.aqua.network/developers/code-examples/prerequisites-and-basics), [pool-discovery guide](https://docs.aqua.network/developers/code-examples/get-pools-info), and [external pools API](https://amm-api.aqua.network/api/external/v1/pools/) | API reported 335 pools | seed the router, ingest its pool lifecycle, and use the API as reconciliation/enrichment |
| Phoenix | [contracts repository](https://github.com/Phoenix-Protocol-Group/phoenix-contracts) and [pinned Mainnet upgrade script](https://github.com/Phoenix-Protocol-Group/phoenix-contracts/blob/aa9bfc000166e3ccdc30281b5181bbd8046fd35d/scripts/upgrade_mainnet.sh) | factory returned 12 pools; script listed 11 | factory state/events win over a stale deployment script; ingest pool and stake contracts together |
| DeFindex | [pinned Mainnet manifest](https://github.com/paltalabs/defindex/blob/a0fcabfae38f1c7b0c9c2f10ff51d225f8edc2f1/public/mainnet.contracts.json), factory state, and [discover API](https://api.defindex.io/vault/discover?network=mainnet) | factory `total_vaults` counter was 107 creations; the public API exposed 14 curated vaults and 14 strategies | replay historical create events because indexed factory entries can expire and require restore; do not assume indices 0–106 are all currently callable; import the seven separately deployed manifest strategies and deprecated strategy mappings |
| Reflector | [orchestrator runtime config](https://orchestrator.reflector.network/config) and [contract source](https://github.com/reflector-network/reflector-contract) | three oracle roots plus subscription, DAO and XRF contracts | snapshot typed feeds/nodes/Wasm hashes and verify each against the ledger; the Lab list omits the forex oracle |
| FxDAO | [official address page](https://fxdao.io/docs/addresses/) and [contract source](https://github.com/FxDAO/FxDAO-SC) | fixed oracle, vault and locking-pool roots | scrape conservatively, timestamp the record, and alert on documented-address changes |

Counts above are observations, not invariants. The sync job must query them again and store `observed_at`, source response hash, and validity range. GitHub-backed records in the seed use immutable commit URLs; each mutable HTTP source represented in the seed carries a response hash from the stated observation date.

## Monitoring potential and decoder priority

DeFiLlama is useful for prioritization, not identity. On 2026-07-18 it attributed approximately $137.8M on Stellar to Blend v2, $45.7M to Aquarius, $17.5M to DeFindex, $7.2M to Templar, $6.0M to Sushi, $1.2M to Soroswap, and $0.5M to Phoenix. These values are volatile and may mix protocol methodology with token/RWA accounting.

The first deep decoder should therefore be **Blend + Reflector + classic SDEX**, because it combines the largest currently attributed lending exposure with a confirmed oracle-manipulation incident. Aquarius and DeFindex follow because of their exposure and large role/child-contract surfaces. Templar and Sushi are high-priority attribution gaps: discover their owned deployments before claiming decoder coverage. Soroswap and Phoenix remain useful early adapters because their factory events and read methods make completeness relatively cheap to prove.

## Stellar Lab and StellarExpert

The official Stellar Lab documents a Smart Contract List with recent contracts on supported networks and a popular-contract view on Mainnet.

The Lab implementation reveals two distinct sources:

- the **Recent** list requests `https://api.stellar.expert/explorer/{network}/contract`;
- the **Popular** list is a hard-coded array in the Lab repository, originally sourced from `stellar-dbt`.

At the time of review, the popular list contained labels for contracts from Band Protocol, Reflector, Soroban Domains, Soroswap, Blend, Aquarius, FxDAO, Phoenix, and several Stellar Asset Contracts. It is useful as an initial curated seed, but it is neither comprehensive nor dynamically authoritative.

Observed unauthenticated StellarExpert endpoints used by Stellar Lab:

| Purpose | Endpoint |
| --- | --- |
| recent contracts | `/explorer/public/contract?limit=...&order=...&cursor=...` |
| contract information | `/explorer/public/contract/{contract_id}` |
| version history | `/explorer/public/contract/{contract_id}/version` |
| Wasm binary | `/explorer/public/wasm/{wasm_hash}` |

The recent-contract response includes the contract ID, creation timestamp, creator, current Wasm hash, invocation/subinvocation counts, event/error counts, and a paging token. Contract information can also include storage-entry counts and validation status.

These contract endpoints are used by the official Lab but are not present in StellarExpert's published OpenAPI document. Treat them as an opportunistic enrichment source: cache results, rate-limit calls, record fetch time, and never make ingestion correctness depend on them.

## Build verification

Stellar's contract lifecycle documentation recommends StellarExpert source validation. A project's GitHub Action builds the Wasm, publishes release artifacts and hashes, and submits an attestation. The Lab warns that “Build Verified” means the build was attested; it does not mean the source code or behavior is safe.

For Stellar Watch, keep these claims separate:

- `code_hash_matches_attested_build`
- `source_repository_known`
- `source_commit_known`
- `protocol_ownership_verified`
- `security_audit_known`
- `security_reviewed_by_ferno`

Never collapse them into one `verified` boolean.

## Attribution confidence

Every protocol-to-contract relationship should store its source and confidence.

| Confidence | Evidence |
| --- | --- |
| authoritative | protocol-owned signed/deployment manifest, official docs plus matching on-chain evidence |
| high | reproducible verified-source attestation and protocol-controlled repository/domain |
| medium | Stellar Lab curated entry, Stellar ecosystem directory, or multiple independent registries |
| low | DeFiLlama/project listing, name similarity, creator clustering, or behavioral inference |
| unknown | discovered only from the ledger |

External records must never overwrite an authoritative mapping silently. Keep validity ranges so migrations and retired contracts remain attributable.

## Registry synchronization design

Create a source adapter interface that yields candidate records:

```text
protocol name
contract ID
contract role
network
source URL
source type
observed/fetched time
claimed Wasm hash (optional)
claimed repository/commit (optional)
confidence
```

Sync candidates into a staging table, validate address/network format, resolve the live on-chain contract and Wasm, then promote or flag them for review. Do not directly replace the canonical registry from a remote response.

Useful automated checks:

- contract exists on the claimed network;
- current and historical Wasm hashes;
- creator and deployment transaction;
- contract meta name/version/source/home-domain fields;
- spec fingerprints and similarity to already attributed contracts;
- calls to or from known protocol factories;
- factory/deployer-created contract clusters;
- protocol site or repository deployment manifests;
- build attestation hash and source commit;
- activity/TVL priority for deciding which protocols receive deep decoders first.

## What can be implemented immediately

1. Load [`stellar-mainnet-defi-roots.json`](../ingester/data/stellar-mainnet-defi-roots.json), preserving source and observation time.
2. Import the Stellar Lab popular list as `source=stellar_lab_popular`, `confidence=medium`, and surface conflicts instead of overwriting protocol-owned roles.
3. Page through StellarExpert's contract API only to compare discovery lag and enrich records; do not make it chain truth.
4. Import the Stellar ecosystem DeFi project list as protocol candidates without inventing contract IDs.
5. Reuse the existing DeFiLlama synchronization idea, filtered to Stellar, for protocol priority and market metadata.
6. Add typed adapters for the protocol sources above, retaining response hashes and deprecated deployments.
7. Reconcile every imported contract against the on-chain inventory and expose source conflicts in an admin review queue.

## Security opportunities from registry data

Once registry records are linked to on-chain facts, Stellar Watch can alert on:

- an official deployment manifest changing to a new contract;
- a known contract moving to an unattested Wasm hash;
- several protocol instances upgrading to a new shared Wasm;
- a factory deploying a new pool or vault implementation;
- a protocol website/repository and the on-chain executable disagreeing;
- a deprecated contract waking up or receiving funds;
- an unknown contract with the same spec/code fingerprint as a known protocol;
- a protocol admin or treasury account weakening its classic multisig policy.

## References

- [Stellar Lab Smart Contract List](https://developers.stellar.org/docs/tools/lab/smart-contracts/smart-contract-list)
- [Stellar Lab popular contract source](https://github.com/stellar/laboratory/blob/c71c08b86e50ab9842fd4af6c1baeb74486b0b1c/src/constants/popularSorobanContracts.ts)
- [Stellar Lab StellarExpert query source](https://github.com/stellar/laboratory/blob/c71c08b86e50ab9842fd4af6c1baeb74486b0b1c/src/query/external/useSEContractsList.ts)
- [Stellar Lab Contract Explorer and build-verification warning](https://developers.stellar.org/docs/tools/lab/smart-contracts/contract-explorer)
- [Stellar contract lifecycle and source validation](https://developers.stellar.org/docs/learn/fundamentals/contract-development/contract-lifecycle)
- [Stellar ecosystem DeFi projects](https://stellar.org/ecosystem?refinementList%5Btags%5D%5B0%5D=DeFi)
- [SDF ecosystem-resources DeFi guide](https://github.com/stellar/ecosystem-resources/blob/main/defi/README.md)
- [Stellar on DeFiLlama](https://defillama.com/chain/stellar)
- [StellarExpert published OpenAPI](https://stellar.expert/openapi)
