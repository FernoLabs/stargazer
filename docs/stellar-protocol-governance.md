# Stellar protocol governance and administration snapshot

This document records the effective administration paths observed for the seven
protocols in Stargazer's initial Stellar DeFi registry. It is an evidence
snapshot for alert-decoder design, not a claim that an address or policy remains
current indefinitely.

## Observation boundary

- Network: Stellar Mainnet.
- Primary observation ledger: `63,676,456`.
- Ledger close time: `2026-07-27T19:15:58Z`.
- DeFindex was re-queried at ledger `63,676,623`, closed
  `2026-07-27T19:31:33Z`; its section identifies the supplemental observations.
- Evidence: current contract storage and interface simulation through Mainnet
  RPC, Classic account signer state through Horizon, and the official sources
  linked below.
- No muxed `M...` administrator was observed.
- A `G...` controller is evaluated from its complete signer and threshold state.
  A `C...` controller is contract governance and must not be labelled as a
  Classic multisig merely because a council participates in its policy.
- Signer ownership and operational independence are unknown unless an official
  source names the operators. Distinct public keys alone do not prove distinct
  people or custody systems.
- Unless a section gives a different signer set, an account classified below as
  `effective single key` had low/medium/high thresholds `0/0/0` and exactly one
  Ed25519 signer: its own master `G...` key at weight 1.

The [Stellar signatures and multisig documentation](https://developers.stellar.org/docs/learn/fundamentals/transactions/signatures-multisig)
is the reference for the classifications below. In particular, a default-style
account with master weight 1 and thresholds 0 still requires transaction
authorization; a master key with weight 0 cannot sign, even against a threshold
of 0.

## Classification summary

| Protocol | Effective controller | Classification | Upgrade model |
| --- | --- | --- | --- |
| Aquarius | `GAV5FBMKD2ZF4X2MGWDNQYUP7KFL7MRM6HZBY7HKQLB4BRHSCCX5J6VS` | strict Classic 2-of-3 for the root Admin, plus separate mostly single-key roles | Admin commit, normally 72-hour wait, Admin apply; EmergencyAdmin may enable an early-apply path |
| Blend v2 | immutable core; one `G...` or `C...` admin per pool | no protocol-wide multisig | new deployments/migrations rather than in-place core or pool upgrades |
| DeFindex | `GAJZKZVWJ237N23KAGBRIFNVE7KOUF27OTIKQPXF3TIJEGYM776WL35P` for the factory; custom roles per vault | factory effective single key; a separate 2-of-3 fee receiver is not a controller | factory changes affect future vaults; an upgradable vault's Manager may upgrade immediately |
| Phoenix | `GCNPDMUMRXBFHMM7KSXU3KVYNM73EDHBFMDK4O33HPY7DA2LXZOZQRDB` | weighted policy with a configured single-signer path | immediate Admin-authorized upgrades |
| Reflector | separate root-admin accounts, each with the same signer policy | strict Classic 4-of-7 | immediate multisig-authorized contract upgrades |
| Soroswap | factory setter and Aggregator Admin are separate `G...` accounts | effective single key for each observed controller | core is not in-place upgradable; Aggregator upgrades are immediate |
| FxDAO | role-specific accounts, currently mostly the same `G...` key | effective single key | immediate role-authorized upgrades and configuration changes |

The YieldBlox Blend v2 pool is included as an important instance of Blend's
per-pool governance. Its pool Admin is a Soroban Governor contract backed by a
strict Classic 4-of-7 council, not a direct multisig controller.

## Aquarius

### Contracts and controllers

- Router: `CBQDHNBFBZYE4MKPWBSJOPIYLW4SFSXAXUTSXJN76GNKYVYPCKWC6QUK`.
- Executable hash at the observation ledger:
  `06f4207b0c9ef78cc595e075ded8fa40e73fdb8346e5f9281068a2d4ba1e5037`.
- Root Admin: [`GAV5FBMKD2ZF4X2MGWDNQYUP7KFL7MRM6HZBY7HKQLB4BRHSCCX5J6VS`](https://horizon.stellar.org/accounts/GAV5FBMKD2ZF4X2MGWDNQYUP7KFL7MRM6HZBY7HKQLB4BRHSCCX5J6VS).
- Root Admin thresholds: low 2, medium 2, high 2.
- Root Admin signers, each weight 1:
  - `GA2YKNLNRAMQ6WXF6D2PD6FKTHFADDYFLJ5N3QGSYG752YFSCGV4I5N7`
  - `GAV5FBMKD2ZF4X2MGWDNQYUP7KFL7MRM6HZBY7HKQLB4BRHSCCX5J6VS`
  - `GDDILLHDLKEB4A7NASCSAJ65V3XAIIEZQK5I6MAJQ3FTVYQYFQGBHXJH`
- Classification: strict Classic 2-of-3. Operator independence is unknown.
- EmergencyAdmin:
  `GCGZ6E5RBUKLNB4VZ5RC65C4QMBSBJ3COVRRJCWAMCXJC36LB7YYWEKM`;
  effective single key.
- Emergency-pause role:
  `GA6MVTGQDCJPP27IAMG6PSDTWOJYTD3NUTLR2W54ADBCBY7OID5YUDSI`;
  effective single key.
- OperationsAdmin:
  `GBVQPX2LQ55HLRMLIWBEYVVQL3SZ5RFPRKYLRSLZU4XRIWAXW2KQIMMD`;
  effective single key.
- PauseAdmin:
  `GA6MA665XVKHTQUZVSMUKUPGT7OREJNCLAZ5ZEH5CXPKYTWFJKZ3YSEK`;
  effective single key.
- Operator:
  `GCXYKA3BM574WC6TWESEDUGUJTNQ5SVCFMHWLQ634H5FTE7FYPV3JH3X`;
  effective single key.
- SystemFeeAdmin:
  `GB57YDVGLL2BAVOXHPXYCZR77J4MLPLMGJKFMTUKMHFI2AEGS4SGGW7N`;
  effective single key.

The current interface exposes `commit_upgrade`, `apply_upgrade`,
`revert_upgrade`, `set_emergency_mode`, ownership-transfer calls, and privileged
role replacement. Upgrade commit records a deadline approximately 259,200
seconds ahead. Admin and EmergencyAdmin ownership transfers also use that delay.
Other privileged roles can be replaced immediately by the root Admin.

Emergency mode does not let EmergencyAdmin replace the contract code directly.
It lets EmergencyAdmin remove the waiting-period check; the 2-of-3 root Admin
must still commit and apply the upgrade.

Alert semantics:

- `commit_upgrade`: high, with the expected executable hash and 72-hour
  countdown.
- emergency mode enabled: critical, because the normal waiting window is no
  longer enforced for apply.
- `apply_upgrade` or any executable-hash change: critical.
- root Admin signer, weight, or threshold change: critical.
- administrator ownership or privileged-role change: high or critical according
  to the role.

Sources:

- [Aquarius Mainnet integration constants](https://docs.aqua.network/developers/code-examples/prerequisites-and-basics)
- [Aquarius Soroban function documentation](https://docs.aqua.network/developers/aquarius-soroban-functions)

Caveat: the formerly public official `AquaToken/soroban-amm` repository returned
404 during this review. Address, role, interface, signer, and delay observations
are high-confidence on-chain evidence, but the current Wasm should be labelled
source-unattested until an authoritative source archive or reproducible release
is pinned.

## Blend v2 and the YieldBlox pool

### Core and pool administration

- Mainnet v2 Pool Factory:
  `CDSYOAVXFY7SM5S64IZPPPYB4GVGGLMQVFREPSQQEZVIWXX5R23G4QSU`.
- Blend's core contracts are deliberately immutable. The factory and lending
  pool code expose no in-place current-contract Wasm update. A version migration
  therefore means a new deployment or pool, not a mutation of the old ID.
- Every pool selects its own `G...` or `C...` Admin.
- Pool Admin transfer is two-step: current Admin proposes, proposed Admin
  accepts. It has no mandatory delay.
- Pool configuration and status actions are immediate.
- Reserve changes are queued for one week outside Setup and can be applied
  immediately while the pool remains in Setup.

Reference FixedV2 pool
`CAJJZSGMMM3PD7N33TAPHGBUGTB43OC73HVIK2L2G6BNGGGYOSSYBXBD` has Admin
[`GAX2VVWVHU5YQY5J3NJBXKHI3FFKZN54BE6GRJCWSIKSBZTQWJJNJMPC`](https://horizon.stellar.org/accounts/GAX2VVWVHU5YQY5J3NJBXKHI3FFKZN54BE6GRJCWSIKSBZTQWJJNJMPC).
The account has master weight 0 and no positive-weight alternate signer. It is a
locked or dead controller, not a multisig.

### YieldBlox contract governance

- YieldBlox Blend v2 pool:
  `CCCCIQSDILITHMM7PBSLVDT5MISSY7R26MNZXCX4H7J5JQ5FPIYOGYFS`.
- Pool Admin / Governor contract:
  `CANSYFVMIP7JVYEZQ463Y2I2VLEVNLDJJ4QNZTDBGLOOGKURPTW4A6FQ`.
- Vote token:
  `CAZ2U7Z2LS3N72NKMOR34HOV4WFDCMA2TPWD3KU4EA2VKZARFPMLF3LI`.
- Current observed policy:
  - vote delay: 34,560 ledgers, approximately two days at five seconds per
    ledger;
  - vote period: 51,840 ledgers, approximately three days;
  - timelock: 34,560 ledgers, approximately two days;
  - grace period: 120,960 ledgers, approximately seven days;
  - quorum: 500 basis points, or 5%;
  - pass threshold: 5,100 basis points, or 51%;
  - proposal threshold: 10,000,000,000 vote-token base units.
- Council:
  [`GBCAS7XIGDRZY4BMABJMGGW7J3YTITRRV5BTEMFQE5ZZSSVWHHX2ZSS4`](https://horizon.stellar.org/accounts/GBCAS7XIGDRZY4BMABJMGGW7J3YTITRRV5BTEMFQE5ZZSSVWHHX2ZSS4).
- Council thresholds: low 4, medium 4, high 4; master weight 0.
- Council signers, each weight 1:
  - `GAMEYWUCSHCK3IOJWA5I5SD7VMWUGR4QZRUL44BYGWCFSGNSMGJRXMEV`
  - `GCPHTH3M3NKMUCTQIOEJI5WFWDUA25YITPUUTNBJWETPRE4VQRNAZWJA`
  - `GCR5PRAEQELWBS52DHYQMR3PJP5VRGLVIRBWF4VS27YFS5XJEQTCROME`
  - `GCXXP5JSZ7V7NG5YFERMQTVHQV3C2PHCWDPTW4CJABIO3BX5RG4K4YWO`
  - `GCZBFC32VGVWW5EZUG4TUPRVIQ5WCQMOB4CRWBY5JLJRCN6DSO7KEDSL`
  - `GDEBZW7AN35W536HGRQRWNIS476HOCEGJID2HROJ5NWKQAMUDR6UBIXC`
  - `GDQF24227KHBERMCJPSL5FZ6JJONH6HIU2NJMG4EDTHRP4QLMXUTIM24`
- Council classification: strict Classic 4-of-7. The pool remains controlled by
  the Governor `C...` contract; the council is one component of that policy.

Only Council can propose a Governor code upgrade, while execution still follows
DAO voting and the timelock. General calldata proposals can invoke the pool with
the Governor contract's authority.

Alert semantics:

- new pool: record Admin, oracle, reserves, implementation family, and factory
  provenance.
- pool Admin proposal and acceptance: critical.
- pool status/configuration/emissions change: high.
- reserve queue: high with a seven-day countdown; reserve apply: high.
- unexpected executable mutation of immutable core or pool code: critical
  invariant violation.
- every Governor proposal: decode type, target contract, function, arguments,
  proposer, voting window, and potential wallet impact.
- passed proposal and executable ETA: high; execution of upgrade, Admin, or risk
  configuration: critical.
- Council signer, weight, or threshold change: critical.

Sources:

- [Blend Mainnet deployments](https://docs.blend.capital/mainnet-deployments)
- [Blend core-contract design](https://docs.blend.capital/tech-docs/core-contracts)
- [Blend lending-pool management](https://docs.blend.capital/tech-docs/core-contracts/lending-pool/pool-management)
- [Pinned Blend Mainnet manifest](https://github.com/blend-capital/blend-utils/blob/b05242df30b6b6caf9d317646f754541824a5a8b/mainnet.contracts.json)
- [Pinned Blend v2 pool source](https://github.com/blend-capital/blend-contracts-v2/blob/ba22b487b2c5057a4ecc28b05b5193c28e4bd117/pool/src/contract.rs)
- [YieldBlox site](https://www.yieldblox.finance/)
- [YieldBlox Governor UI](https://mainnet.governance.script3.io/CANSYFVMIP7JVYEZQ463Y2I2VLEVNLDJJ4QNZTDBGLOOGKURPTW4A6FQ/proposals/)
- [Pinned Soroban Governor architecture](https://github.com/script3/soroban-governor/blob/a2ac6de81055be5bd13e31f922c9546309bfdb8a/docs/architecture.md)

Caveats: the two queried pools used executable hash
`a41fc53d6753b6c04eb15b021c55052366a4c8e0e21bc72700f461264ec1350e`
at the observation ledger, which differs from an older manifest hash. It is an
observed hash, not source-attested here. YieldBlox's legacy governance
documentation states a 60% threshold, but current contract state resolves to
51%; timestamped on-chain policy takes precedence in this snapshot.

## DeFindex

### Contracts and controllers

- Factory:
  `CDKFHFJIET3A73A2YN4KV7NSV32S6YGQMUFH3DNJXLBWL4SKEGVRNFKI`.
- Factory Admin:
  [`GAJZKZVWJ237N23KAGBRIFNVE7KOUF27OTIKQPXF3TIJEGYM776WL35P`](https://horizon.stellar.org/accounts/GAJZKZVWJ237N23KAGBRIFNVE7KOUF27OTIKQPXF3TIJEGYM776WL35P).
- Factory Admin signer state: low/medium/high thresholds `0/0/0`; one Ed25519
  signer, the master `GAJZKZ...L35P`, at weight 1.
- Factory Admin classification: effective single key at the observation ledger.
- Protocol fee receiver:
  [`GCTQL76FD4EHJJ6V7NQO4BA2CHRALBLEVPD5EANNQYIOZLO3LIW7EAQ6`](https://horizon.stellar.org/accounts/GCTQL76FD4EHJJ6V7NQO4BA2CHRALBLEVPD5EANNQYIOZLO3LIW7EAQ6).
- Fee-receiver thresholds: low 2, medium 2, high 2; master weight 0.
- Fee-receiver signers, each weight 1:
  - `GAHF3QTOKBQ6HZ2J3XS4KMFV5E5IBDESG3A4JGCLBLOUKTEYGERIPPER`
  - `GDFDUYZJMVZYIJN7WPCNVFGBAXWGNH7ABEDIXEKK7IAYIZVXTSJ63HYK`
  - `GDOFVJK6WW2QXGBY6VTFXZROQXGK2TVL5KC6T6DV7ZJK57OBGDZDCV5B`
- Fee-receiver classification: strict Classic 2-of-3, but it receives protocol
  fees and is not a factory or vault controller. It must not be used as the
  protocol's governance badge.
- Factory state at the supplemental observation ledger: protocol fee 5,000
  basis points, future-vault Wasm hash
  `ae3409a4090bc087b86b4e9b444d2b8017ccd97b90b069d44d005ab9f8e1468b`,
  and `total_vaults = 108`.
- Each vault independently defines Manager, EmergencyManager, FeeReceiver, and
  RebalanceManager roles. A vault must therefore display its own controllers;
  the factory Admin is not a valid substitute for vault governance.
- Factory Admin can replace itself and change the protocol receiver, fee, and
  future-vault Wasm immediately; there is no two-step acceptance or delay. These
  settings affect future vault deployments rather than changing existing vault
  roles or fees.
- `upgradable` is selected per vault at construction. If enabled, Manager can
  upgrade immediately without a mandatory timelock. Manager can replace every
  operational role, including itself, in one step. Manager and EmergencyManager
  can pause, unpause, or rescue strategies; RebalanceManager can execute vault
  rebalance instructions; FeeReceiver can replace itself under the documented
  role checks.
- All seven strategies in the pinned official manifest report keeper
  `GC7LSNNCRW4PNLLHIXE53RB7N5BKSQVXTGWYHIYBA7LAWY7XYPC4JGCP`, an effective
  single-key account. Keeper can harvest and transfer the keeper role
  immediately. The current strategy source has no code-upgrade entry point.

Alert semantics:

- Factory Admin signer/policy change: critical.
- Factory Admin or future-vault Wasm/configuration change: high or critical.
- Vault creation: persist all role addresses, asset configuration, strategies,
  executable hash, and source factory.
- Any vault role change, Manager-authorized upgrade, emergency pause/unpause, or
  strategy rescue: high or critical, with the acting authorization path.
- Strategy Keeper change: high; unexpected strategy executable change: critical
  invariant violation.
- Strategy add/remove, rebalance, harvest, and dependency-address changes:
  decode separately from a code upgrade.

Sources:

- [Pinned current DeFindex Mainnet manifest](https://github.com/defindex-io/stellar-contracts/blob/7dd422786e1d764f06fb1d96ca7a2fc375731237/public/mainnet.contracts.json)
- [Pinned DeFindex factory source](https://github.com/defindex-io/stellar-contracts/blob/7dd422786e1d764f06fb1d96ca7a2fc375731237/factory/src/lib.rs)
- [Pinned DeFindex vault source](https://github.com/defindex-io/stellar-contracts/blob/7dd422786e1d764f06fb1d96ca7a2fc375731237/vault/src/lib.rs)
- [Pinned DeFindex contract architecture and roles](https://github.com/defindex-io/stellar-contracts/blob/7dd422786e1d764f06fb1d96ca7a2fc375731237/docs/defindex-contracts.md)
- [Archived manifest used to seed the local registry](https://github.com/paltalabs/defindex/blob/a0fcabfae38f1c7b0c9c2f10ff51d225f8edc2f1/public/mainnet.contracts.json)
- [DeFindex official Mainnet deployments](https://docs.defindex.io/getting-started/getting-started/contract-deployments/mainnet-deployment)
- [DeFindex official vault-creation and role guide](https://docs.defindex.io/api-integration-guide/creating-a-defindex-vault)
- [DeFindex Mainnet discovery API](https://api.defindex.io/vault/discover?network=mainnet)

Caveat: controller details are per vault and can diverge. The factory's creation
counter is not a complete list of currently callable vault indices because
archived entries may require restoration. Historical factory events and current
vault state must be reconciled before claiming full controller coverage. The
`paltalabs/defindex` monorepo was archived in July 2026 and points to
`defindex-io/stellar-contracts`; both manifests identify the same factory and
seven published strategy deployments. Documentation recommends multisig Manager
accounts but the contract architecture does not enforce multisig; each live role
address must be resolved and classified independently. Some documentation still
links the archived repository or an older deployment set, so the pinned current
manifest and on-chain state take precedence here.

## Phoenix

### Contracts and controller

- Factory: `CB4SVAWJA6TSRNOJZ7W2AWFW46D5VR4ZMFZKDIKXEINZCZEGZCJZCKMI`.
- Multihop: `CCLZRD4E72T7JCZCN3P7KNPYNXFYKQCL64ECLX7WP5GNVYPYJGU2IO2G`.
- Vesting: `CDEGWCGEMNFZT3UUQD7B4TTPDHXZLGEDB6WIP4PWNTXOR5EZD34HJ64O`.
- Observed Admin across the queried factory, multihop, pool, and vesting
  contracts:
  [`GCNPDMUMRXBFHMM7KSXU3KVYNM73EDHBFMDK4O33HPY7DA2LXZOZQRDB`](https://horizon.stellar.org/accounts/GCNPDMUMRXBFHMM7KSXU3KVYNM73EDHBFMDK4O33HPY7DA2LXZOZQRDB).
- Thresholds: low 20, medium 20, high 20.
- Signers:
  - `GA7B3D2FROSMC6YGTXN6M4MAA6K5ZTA4CQCJRUFNUOPWJ4ELQTMVMFRO`, weight 20;
  - `GAFZL4ET65XLKSAZBFVWSPFRQCMKDQR3VXNT6RAOO4325PNFRSPCJQL6`, weight 10;
  - `GCNPDMUMRXBFHMM7KSXU3KVYNM73EDHBFMDK4O33HPY7DA2LXZOZQRDB`, weight 10.
- Classification: weighted Classic policy. The weight-20 signer can authorize
  alone; alternatively, the two weight-10 signers can combine. It is not a
  strict 2-of-3.

Factory, multihop, pool, and vesting code supports immediate Admin-authorized
updates. Admin transfer is two-step proposal and acceptance. Its `time_limit` is
an expiration window, not a timelock; acceptance can happen immediately.

Alert semantics:

- any executable change: critical.
- Admin-transfer proposal: high; acceptance: critical.
- signer, weight, or threshold change: critical, especially a new single-signer
  authorization path.
- factory child-Wasm or deployment configuration change: high.

Sources:

- [Pinned Phoenix Mainnet deployment and upgrade script](https://github.com/Phoenix-Protocol-Group/phoenix-contracts/blob/aa9bfc000166e3ccdc30281b5181bbd8046fd35d/scripts/upgrade_mainnet.sh)
- [Pinned Phoenix factory source](https://github.com/Phoenix-Protocol-Group/phoenix-contracts/blob/aa9bfc000166e3ccdc30281b5181bbd8046fd35d/contracts/factory/src/contract.rs)

Confidence is high for the observed policy and source behavior. Operator
independence is unknown.

## Reflector

### Contracts and controllers

| Root | Contract | Admin |
| --- | --- | --- |
| Crypto/USD oracle | `CAFJZQWSED6YAWZU3GWRTOCNPPCGBN32L7QV43XX5LZLFTK6JLN34DLN` | `GCJ2RDQLOVNOYBD2LELEGEVIDZ3LRLPHGANVOFJZXTUQHKHVZX2LMRFB` |
| Stellar-assets oracle | `CALI2BYU2JE6WVRUFYTS6MSBNEHGJ35P4AVCZYF3B6QOE3QKOB2PLE6M` | `GC2M2UTZ57GSENZBTPMLPB6QSAL2USIPHMXSUCT2I3V24GYWF3EI6RFA` |
| Forex oracle | `CBKGPWGKSKZF52CFHMTRR23TBWTPMRDIYZ4O2P5VS65BMHYH4DXMCJZC` | `GCSJ7NZKNRP2NXUOOPHGNNPGOEMUKGUNPLQ4NOW2B2L4X7WEO6NQFXRF` |
| Subscriptions | `CBNGTWIVRCD4FOJ24FGAKI6I5SDAXI7A4GWKSQS7E6UYSR4E4OHRI2JX` | `GBSLWQGVZTCACYLAHN4QIZWI4H7JNCW43PAQKBBMRZGF46AMU7EARFSS` |
| DAO | `CBQSUF57OYX4RIMCZV62DKN6JFOTEKPHIZASMJYOUOCNHGNG2P3XQLSE` | `GDSRHC7U5XWNGHDML6SBU7HXLH67ILO7PCCCFMN55PPBYYCZEAU4FXRF` |

Every listed Admin has master weight 0 and thresholds low 4, medium 4, high 4.
Every one has the same seven weight-1 signers:

- `GACBOG4ELLJWKC4CDEML4LHEFI4IZZNEMW2QBQUI6JKFHNWISLHGTZ2I`
- `GAWZMAJFWF36UIVYUZULZL5DUMHT3CEWHXNG43GMENS7ZBF42YY42WXC`
- `GCLYRJICX4GX76XODYCJHTVAYHS7QC766YXVBUWRXCYC4H6ZMC6ESECP`
- `GDDD5IXECVO6PV5FYTANKQV7QPB2TYPIJAT7NBALMYNN6UXM75HBSAIL`
- `GDEAZIGD6LFY64O7PD5MIPDWY2WAGSWZHEUEUSFKK2T3MBBBYQMPSER4`
- `GDEPSRTBCPGNZP7TI4GVFG7VAFXCQTJYUH3PV5RVD4QZSNDWHCX73LO4`
- `GDXEKG6NJIBRJUOJ32S2EREP3SPIOORA3XEMQCRLCP7GOAGKAFXCQQOO`

Classification: strict Classic 4-of-7. Reflector's runtime configuration names
the corresponding operator domains as `creit.tech`, `stellar.expert`,
`publicnode.org`, `lightsail.network`, `xycloo.com`, `ultrastellar.com`, and
`script3.io`. This is stronger attribution than key-count inference, although it
does not cryptographically prove operational independence.

Oracle Admin can authorize prices, feed/configuration changes, assets, and an
immediate `update_contract(wasm_hash)`; there is no mandatory Wasm-upgrade
timelock. DAO ballot state is distinct from the Classic multisig account that
authorizes contract actions.

Alert semantics:

- signer, weight, or threshold change on any root Admin: critical.
- executable, Admin, feed, source, or asset-set change: high or critical.
- an authorized price update: normal telemetry; an unexpected authorization or
  signer-policy deviation: critical.
- DAO ballot/proposal and resulting execution: separate findings with the target
  action decoded.

Sources:

- [Reflector live runtime configuration](https://orchestrator.reflector.network/config)
- [Pinned Reflector oracle source](https://github.com/reflector-network/reflector-contract/blob/42f3116b0c5ea335181508c926e5ec48d67af419/oracle/src/price_oracle.rs)
- [Reflector DAO blueprint](https://reflector.network/dao/blueprint)

Confidence is high for the roots, signer policy, named operator domains, and
immediate upgrade behavior at the observation ledger.

## Soroswap

### Core and Aggregator controllers

- Factory: `CA4HEQTL2WPEUYKYKCDOHCDNIV4QHNJ7EL4J4NQ6VADP7SYHVRYZ7AW2`.
- Router: `CAG5LRYQ5JVEUI5TEID72EYOVX44TTUJT5BQR2J6J77FH65PCCFAJDDH`.
- Factory fee setter:
  [`GAYPUMZFDKUEUJ4LPTHVXVG2GD5B6AV5GGLYDMSZXCSI4QILQKSY25JI`](https://horizon.stellar.org/accounts/GAYPUMZFDKUEUJ4LPTHVXVG2GD5B6AV5GGLYDMSZXCSI4QILQKSY25JI),
  effective single key.
- Aggregator:
  `CAYP3UWLJM7ZPTUKL6R6BFGTRWLZ46LRKOXTERI2K6BIJAWGYY62TXTO`.
- Aggregator Admin:
  [`GDC6LYIWADQM6XSUQEZDHNA6C4DVPRNYJ2OIH5HB67WBXBWVC6YRV6LC`](https://horizon.stellar.org/accounts/GDC6LYIWADQM6XSUQEZDHNA6C4DVPRNYJ2OIH5HB67WBXBWVC6YRV6LC),
  effective single key.

Core factory/router/pair source exposes no in-place upgrade entry point. The fee
setter can change fee recipient, fee enablement, and the setter address. The
Aggregator Admin can immediately replace the Admin, upgrade code, and update,
remove, or pause adapters; no mandatory timelock or two-step acceptance was
observed.

Alert semantics:

- fee recipient or fee switch: high.
- fee-setter transfer: critical.
- Aggregator Admin or signer-policy change: critical.
- Aggregator executable update: critical.
- adapter update, removal, or pause: high.
- any core executable mutation: critical invariant violation.

Sources:

- [Pinned Soroswap core Mainnet manifest](https://github.com/soroswap/core/blob/bb90a65556d8eee0dc698ac75de0f280e547fedc/public/mainnet.contracts.json)
- [Pinned Soroswap factory source](https://github.com/soroswap/core/blob/bb90a65556d8eee0dc698ac75de0f280e547fedc/contracts/factory/src/lib.rs)
- [Pinned Soroswap Aggregator Mainnet manifest](https://github.com/soroswap/aggregator/blob/84de10e0f8d26168b4a76f8c23b963e50917517c/public/mainnet.contracts.json)
- [Pinned Soroswap Aggregator source](https://github.com/soroswap/aggregator/blob/84de10e0f8d26168b4a76f8c23b963e50917517c/contracts/aggregator/src/lib.rs)

Confidence is high for both observed controller accounts and source-defined
permissions. No multisig was observed.

## FxDAO

### Contracts and controllers

- Vaults:
  `CCUN4RXU5VNDHSF4S4RKV4ZJYMX2YWKOH6L4AKEKVNVDQ7HY5QIAO4UB`.
- Vault Admin and ProtocolManager:
  [`GC7JVOXZJSHY3GHKWUJWKIUYWEJ4RZABSRQZQ5JBZEC5QUTYBUHVNIKV`](https://horizon.stellar.org/accounts/GC7JVOXZJSHY3GHKWUJWKIUYWEJ4RZABSRQZQ5JBZEC5QUTYBUHVNIKV),
  effective single key.
- Locking pool:
  `CDCART6WRSM2K4CKOAOB5YKUVBSJ6KLOVS7ZEJHA4OAQ2FXX7JOHLXIP`.
- Locking-pool Admin and Manager: the same `GC7J...NIKV` account at the
  observation ledger.
- Oracle:
  `CB5OTV4GV24T5USEZHFVYGC3F4A4MPUQ3LN56E76UK2IT7MJ6QXW4TFS`.
- Oracle storage Admin:
  [`GCOMGAD2XPXGHLSHRUY4GFTIKIZJX3V67TI7EXVOVCWF7QIIJ7AANZTP`](https://horizon.stellar.org/accounts/GCOMGAD2XPXGHLSHRUY4GFTIKIZJX3V67TI7EXVOVCWF7QIIJ7AANZTP),
  effective single key.

The vault Admin controls immediate code upgrade, fees, and Admin replacement.
ProtocolManager controls panic mode, oracle address, ProtocolManager replacement,
currencies, and vault conditions. The locking-pool Manager controls immediate
upgrade and pool changes. The vault reported `panic_mode = true` at the
observation ledger.

Alert semantics:

- any executable, Admin, ProtocolManager, Manager, signer, or threshold change:
  critical.
- panic-mode transition: high or critical according to direction and context.
- oracle address, supported currency, collateral, liquidation, or other risk
  configuration change: high.
- retain role identity even while multiple roles resolve to one account.

Sources:

- [FxDAO official address registry](https://fxdao.io/docs/addresses/)
- [Pinned FxDAO vault source](https://github.com/FxDAO/FxDAO-SC/blob/b73d8b6c53a1f241f858bf6631ea80ed56300a7f/contracts/vaults/src/contract.rs)
- [Pinned FxDAO locking-pool source](https://github.com/FxDAO/FxDAO-SC/blob/b73d8b6c53a1f241f858bf6631ea80ed56300a7f/contracts/locking-pool/src/contract.rs)

Confidence is high for the documented vault and locking-pool roles. The Oracle
Admin is high-confidence live on-chain state but only medium-high protocol-source
attribution because FxDAO's address page does not identify that Admin account.

## Required monitoring model

The protocol page and alert engine should preserve these distinctions instead of
using a single `multisig` flag:

1. Controller kind: `classic_account`, `contract_governor`,
   `locked_account`, or `immutable_no_admin`.
2. For every Classic controller: low/medium/high thresholds, master weight, full
   signer set and signer types, signer weights, smallest satisfying signer sets,
   and whether any single-signer path exists.
3. For every contract Governor: council/controller, proposal types, targets,
   voting delay and period, quorum, pass threshold, timelock, grace period, and
   execution state.
4. For every protocol role: the contract, controller, source, observation ledger,
   first and last validity ledger, and confidence.
5. For every executable version: contract ID, role, executable hash, first and
   last observed ledger/time, transition transaction, upgrade mechanism, and the
   controller policy effective at the transition.
6. For immutable systems: migration lineage between contract IDs without
   describing the old contract as upgraded.

Alert generation must consume both Soroban activity and Classic `set_options`
operations. An executable change is only one part of the risk surface: a signer,
threshold, Admin, emergency role, oracle, adapter, or governance-policy change
can be the more important precursor.
