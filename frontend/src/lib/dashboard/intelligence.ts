export interface IntelligenceSource {
	readonly label: string;
	readonly url: string;
}

export interface VerifiedIncident {
	readonly id: string;
	readonly occurredAt: string;
	readonly title: string;
	readonly classification: 'protocol-exploit' | 'network-state-integrity';
	readonly severity: 'critical' | 'high';
	readonly impact: string;
	readonly summary: string;
	readonly protocols: readonly string[];
	readonly ledger: number | null;
	readonly primaryContract: string | null;
	readonly evidenceTransaction: string | null;
	readonly archiveStatus: string;
	readonly sources: readonly IntelligenceSource[];
}

export const VERIFIED_INCIDENTS: readonly VerifiedIncident[] = [
	{
		id: 'yieldblox-oracle-manipulation-2026-02-22',
		occurredAt: '2026-02-22T00:22:09Z',
		title: 'YieldBlox DAO pool oracle manipulation',
		classification: 'protocol-exploit',
		severity: 'critical',
		impact: 'Approximately $10.2M gross drain; about $7.3M in XLM was later quarantined.',
		summary:
			'An attacker moved the price in a near-empty USTRY/USDC order book, waited through two oracle windows, supplied the inflated asset to a YieldBlox pool on Blend V2, and borrowed most available liquidity.',
		protocols: ['YieldBlox', 'Blend v2', 'Reflector'],
		ledger: 61_340_315,
		primaryContract: 'CCCCIQSDILITHMM7PBSLVDT5MISSY7R26MNZXCX4H7J5JQ5FPIYOGYFS',
		evidenceTransaction:
			'56466ad66cd5c49a251b9f1c7c0cf6b1b2f62d121b58ff856e2b43673b22be51',
		archiveStatus:
			'The historical correlation rule reproduces the exact wallet-to-pool relationship. General behavioral detection still requires SDEX, Reflector, and Blend event replay.',
		sources: [
			{
				label: 'BlockSec forensic report',
				url: 'https://blocksec.com/blog/yieldblox-dao-incident-on-stellar-oracle-misconfiguration-enabled-a-10m-drain'
			},
			{
				label: 'Blockaid containment report',
				url: 'https://www.blockaid.io/blog/73-quarantined-how-blockaid-and-stellar-validators-contained-a-10m-price-manipulation-attack'
			},
			{
				label: 'Stellar ecosystem acknowledgement',
				url: 'https://stellar.org/blog/ecosystem/what-the-defi-is-happening-on-stellar'
			}
		]
	},
	{
		id: 'protocol-23-state-archival-2025',
		occurredAt: '2025-09-04T00:00:00Z',
		title: 'Protocol 23 state-archival corruption',
		classification: 'network-state-integrity',
		severity: 'high',
		impact: '478 persistent entries were affected; this was a data-integrity incident, not a theft.',
		summary:
			'Stale restored values corrupted archived state and some token events were omitted. Registered roots touched by the affected key set include Blend v2, FxDAO, and Reflector contracts.',
		protocols: ['Blend v2', 'FxDAO', 'Reflector'],
		ledger: 58_762_517,
		primaryContract: null,
		evidenceTransaction: null,
		archiveStatus:
			'Archive provenance verified: all 12 overlapping partitions are the corrected stellar-core 24.1.0 re-export. Executable hashes were unaffected; full token-event payloads still require a dedicated reparse.',
		sources: [
			{
				label: 'SDF post-mortem',
				url: 'https://stellar.org/blog/foundation-news/state-archival-issue-post-mortem'
			},
			{
				label: 'SDF re-export instructions',
				url: 'https://stellar.org/blog/developers/reconciling-stellar-events'
			}
		]
	}
] as const;

export interface GovernanceControllerSnapshot {
	readonly id: string;
	readonly protocol: string;
	readonly surface: string;
	readonly kind:
		| 'strict-multisig'
		| 'weighted-policy'
		| 'contract-governor'
		| 'single-key'
		| 'locked'
		| 'immutable-core'
		| 'verification-pending';
	readonly controllerAddress: string | null;
	readonly policy: string;
	readonly upgradeMechanism: string;
	readonly alertRule: string;
	readonly source: IntelligenceSource;
}

export const GOVERNANCE_SNAPSHOT = {
	ledger: 63_676_623,
	observedAt: '2026-07-27T19:31:33Z',
	controllers: [
		{
			id: 'aquarius-router-admin',
			protocol: 'Aquarius',
			surface: 'AMM router admin',
			kind: 'strict-multisig',
			controllerAddress: 'GAV5FBMKD2ZF4X2MGWDNQYUP7KFL7MRM6HZBY7HKQLB4BRHSCCX5J6VS',
			policy: 'Strict 2-of-3 classic Stellar account',
			upgradeMechanism: 'Admin commit, 72-hour delay, then apply; emergency mode can bypass waiting.',
			alertRule: 'Signer/threshold change or emergency mode: critical. Upgrade commit: high.',
			source: {
				label: 'Aquarius Soroban functions',
				url: 'https://docs.aqua.network/developers/aquarius-soroban-functions'
			}
		},
		{
			id: 'blend-core',
			protocol: 'Blend v2',
			surface: 'Core factory and pools',
			kind: 'immutable-core',
			controllerAddress: 'CDSYOAVXFY7SM5S64IZPPPYB4GVGGLMQVFREPSQQEZVIWXX5R23G4QSU',
			policy: 'No in-place Wasm update; each pool selects its own G- or C-address admin.',
			upgradeMechanism: 'New deployment and migration rather than executable replacement.',
			alertRule: 'New pool plus decoded admin/oracle; any core executable mutation is critical.',
			source: {
				label: 'Blend core contracts',
				url: 'https://docs.blend.capital/tech-docs/core-contracts'
			}
		},
		{
			id: 'yieldblox-governor',
			protocol: 'YieldBlox / Blend v2',
			surface: 'Exploited pool administrator',
			kind: 'contract-governor',
			controllerAddress: 'CANSYFVMIP7JVYEZQ463Y2I2VLEVNLDJJ4QNZTDBGLOOGKURPTW4A6FQ',
			policy: 'Governor contract backed by a strict 4-of-7 classic-account council.',
			upgradeMechanism: 'About 2-day vote delay, 3-day vote, and 2-day timelock.',
			alertRule: 'Proposal target/function, council policy, admin acceptance, and execution: critical.',
			source: {
				label: 'Governor architecture',
				url: 'https://github.com/script3/soroban-governor/blob/a2ac6de81055be5bd13e31f922c9546309bfdb8a/docs/architecture.md'
			}
		},
		{
			id: 'fxdao-vaults-admin',
			protocol: 'FxDAO',
			surface: 'Vaults admin and protocol manager',
			kind: 'single-key',
			controllerAddress: 'GC7JVOXZJSHY3GHKWUJWKIUYWEJ4RZABSRQZQ5JBZEC5QUTYBUHVNIKV',
			policy: 'The two immediate roles currently resolve to the same single-key account.',
			upgradeMechanism: 'Immediate code, fee, oracle, panic-mode, and risk configuration actions.',
			alertRule: 'Executable, admin, manager, or signer-policy change: critical.',
			source: {
				label: 'FxDAO addresses',
				url: 'https://fxdao.io/docs/addresses/'
			}
		},
		{
			id: 'phoenix-admin',
			protocol: 'Phoenix',
			surface: 'Factory, router, pools, and vesting admin',
			kind: 'weighted-policy',
			controllerAddress: 'GCNPDMUMRXBFHMM7KSXU3KVYNM73EDHBFMDK4O33HPY7DA2LXZOZQRDB',
			policy: 'Threshold 20: one weight-20 signer can act alone, or both weight-10 signers together.',
			upgradeMechanism: 'Immediate admin-auth updates; transfer proposal has expiry, not a timelock.',
			alertRule: 'Executable, signer, threshold, or accepted admin transfer: critical.',
			source: {
				label: 'Phoenix upgrade script',
				url: 'https://github.com/Phoenix-Protocol-Group/phoenix-contracts/blob/aa9bfc000166e3ccdc30281b5181bbd8046fd35d/scripts/upgrade_mainnet.sh'
			}
		},
		{
			id: 'reflector-root-admins',
			protocol: 'Reflector',
			surface: 'Five official root administrators',
			kind: 'strict-multisig',
			controllerAddress: 'GC2M2UTZ57GSENZBTPMLPB6QSAL2USIPHMXSUCT2I3V24GYWF3EI6RFA',
			policy: 'Each root admin is a strict 4-of-7 account with the same seven weight-1 signers.',
			upgradeMechanism: 'Immediate admin-auth code and oracle configuration updates.',
			alertRule: 'Signer/threshold deviation or executable/admin change: critical.',
			source: {
				label: 'Reflector current mainnet config',
				url: 'https://orchestrator.reflector.network/config'
			}
		},
		{
			id: 'soroswap-core',
			protocol: 'Soroswap',
			surface: 'Core factory/router and aggregator',
			kind: 'single-key',
			controllerAddress: 'GDC6LYIWADQM6XSUQEZDHNA6C4DVPRNYJ2OIH5HB67WBXBWVC6YRV6LC',
			policy: 'Core is non-upgradeable; aggregator administration is a separate single-key account.',
			upgradeMechanism: 'Aggregator can immediately upgrade and mutate or pause adapters.',
			alertRule: 'Aggregator admin or executable change: critical; adapter mutation: high.',
			source: {
				label: 'Soroswap aggregator manifest',
				url: 'https://github.com/soroswap/aggregator/blob/84de10e0f8d26168b4a76f8c23b963e50917517c/public/mainnet.contracts.json'
			}
		},
		{
			id: 'defindex-factory-admin',
			protocol: 'DeFindex',
			surface: 'Factory admin; vault roles remain independent',
			kind: 'single-key',
			controllerAddress: 'GAJZKZVWJ237N23KAGBRIFNVE7KOUF27OTIKQPXF3TIJEGYM776WL35P',
			policy: 'Factory Admin is single-key. A separate strict 2-of-3 fee receiver is not a controller.',
			upgradeMechanism: 'Immediate future-vault Wasm/config changes; an upgradable vault Manager may upgrade immediately.',
			alertRule: 'Factory policy or future Wasm: critical; every vault role and Manager upgrade: high or critical.',
			source: {
				label: 'DeFindex current manifest',
				url: 'https://github.com/defindex-io/stellar-contracts/blob/7dd422786e1d764f06fb1d96ca7a2fc375731237/public/mainnet.contracts.json'
			}
		}
	] satisfies readonly GovernanceControllerSnapshot[]
} as const;
