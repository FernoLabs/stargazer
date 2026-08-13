import type { DashboardRepository, LeaderboardQuery } from './repository';
import { parseStellarWalletAddress } from './stellar';
import type {
	DashboardDatasetMetadata,
	Leaderboard,
	LeaderboardRow,
	ProtocolInteraction,
	StellarAccountAddress,
	StellarContractAddress,
	StellarMuxedAddress,
	StellarTransactionHash,
	WalletAlert,
	WalletDashboard
} from './types';

/**
 * DETERMINISTIC DEMO DATA ONLY.
 *
 * None of the counts, relationships, timestamps, or alerts in this file are
 * claims about the named wallets or protocols. The compact-index API should
 * replace this repository when the production query layer is available.
 */
export const DEMO_DATASET_METADATA: DashboardDatasetMetadata = {
	mode: 'demo',
	label: 'Synthetic product preview',
	asOf: '2026-07-25T18:00:00.000Z',
	network: 'Stellar Mainnet',
	indexedLedgerStart: 57_280_000,
	indexedLedgerEnd: 63_615_999,
	disclaimer: 'Deterministic demo data — not a representation of real wallet activity or alerts.'
};

export const DEMO_WALLET_ADDRESS = demoAccount(
	'GA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQHES5'
);

/** Valid M-address alias for DEMO_WALLET_ADDRESS, useful for testing normalization. */
export const DEMO_MUXED_WALLET_ADDRESS = demoMuxedAccount(
	'MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU'
);

const DEMO_PROTOCOLS: readonly ProtocolInteraction[] = [
	{
		protocolId: 'blend-v2',
		protocolName: 'Blend',
		category: 'lending',
		attributionConfidence: 'authoritative',
		transactionCount: 9_284,
		successfulTransactionCount: 9_201,
		contractCount: 3,
		firstInteractionAt: '2025-08-04T09:18:21.000Z',
		lastInteractionAt: '2026-07-25T17:42:08.000Z',
		lastTransactionHash: demoHash('d1'.repeat(32)),
		contracts: [
			demoContract('CD25MNVTZDL4Y3XBCPCJXGXATV5WUHHOWMYFF4YBEGU5FCPGMYTVG5JY'),
			demoContract('CCOQM6S7ICIUWA225O5PSJWUBEMXGFSSW2PQFO6FP4DQEKMS5DASRGRR'),
			demoContract('CAQQR5SWBXKIGZKPBZDH3KM5GQ5GUTPKB7JAFCINLZBC5WXPJKRG3IM7')
		]
	},
	{
		protocolId: 'soroswap',
		protocolName: 'Soroswap',
		category: 'exchange',
		attributionConfidence: 'authoritative',
		transactionCount: 4_611,
		successfulTransactionCount: 4_542,
		contractCount: 3,
		firstInteractionAt: '2025-09-12T14:03:44.000Z',
		lastInteractionAt: '2026-07-24T21:14:19.000Z',
		lastTransactionHash: demoHash('a7'.repeat(32)),
		contracts: [
			demoContract('CA4HEQTL2WPEUYKYKCDOHCDNIV4QHNJ7EL4J4NQ6VADP7SYHVRYZ7AW2'),
			demoContract('CAG5LRYQ5JVEUI5TEID72EYOVX44TTUJT5BQR2J6J77FH65PCCFAJDDH'),
			demoContract('CAYP3UWLJM7ZPTUKL6R6BFGTRWLZ46LRKOXTERI2K6BIJAWGYY62TXTO')
		]
	},
	{
		protocolId: 'aquarius',
		protocolName: 'Aquarius',
		category: 'liquidity',
		attributionConfidence: 'authoritative',
		transactionCount: 2_247,
		successfulTransactionCount: 2_232,
		contractCount: 2,
		firstInteractionAt: '2025-11-23T06:52:09.000Z',
		lastInteractionAt: '2026-07-23T11:06:51.000Z',
		lastTransactionHash: demoHash('5b'.repeat(32)),
		contracts: [
			demoContract('CBQDHNBFBZYE4MKPWBSJOPIYLW4SFSXAXUTSXJN76GNKYVYPCKWC6QUK'),
			demoContract('CDHDUKHFZB6FORHEBZCNYI3GGVNVOLEITSGI7OKU4UIQND5QG75KGSRR')
		]
	},
	{
		protocolId: 'reflector',
		protocolName: 'Reflector',
		category: 'oracle',
		attributionConfidence: 'authoritative',
		transactionCount: 1_046,
		successfulTransactionCount: 1_044,
		contractCount: 2,
		firstInteractionAt: '2026-01-08T17:31:12.000Z',
		lastInteractionAt: '2026-07-21T15:58:27.000Z',
		lastTransactionHash: demoHash('3e'.repeat(32)),
		contracts: [
			demoContract('CAFJZQWSED6YAWZU3GWRTOCNPPCGBN32L7QV43XX5LZLFTK6JLN34DLN'),
			demoContract('CALI2BYU2JE6WVRUFYTS6MSBNEHGJ35P4AVCZYF3B6QOE3QKOB2PLE6M')
		]
	}
];

const DEMO_ALERTS: readonly WalletAlert[] = [
	{
		id: 'demo-alert-003',
		kind: 'new-protocol-interaction',
		severity: 'info',
		title: 'New protocol relationship discovered',
		summary: 'The demo wallet interacted with a mapped Reflector oracle contract for the first time.',
		detectedAt: '2026-07-21T15:58:31.000Z',
		ledgerSequence: 63_580_412,
		protocolId: 'reflector',
		protocolName: 'Reflector',
		contractAddress: demoContract(
			'CAFJZQWSED6YAWZU3GWRTOCNPPCGBN32L7QV43XX5LZLFTK6JLN34DLN'
		),
		transactionHash: demoHash('3e'.repeat(32)),
		deliveries: [
			{ channel: 'telegram', status: 'sent', sentAt: '2026-07-21T15:58:34.000Z' },
			{ channel: 'email', status: 'sent', sentAt: '2026-07-21T15:58:39.000Z' }
		]
	},
	{
		id: 'demo-alert-002',
		kind: 'contract-executable-change',
		severity: 'medium',
		title: 'Observed contract executable change',
		summary: 'A mapped demo dependency moved to a newly observed Wasm hash; the transaction is retained as evidence.',
		detectedAt: '2026-07-16T08:17:03.000Z',
		ledgerSequence: 63_491_608,
		protocolId: 'blend-v2',
		protocolName: 'Blend',
		contractAddress: demoContract(
			'CCOQM6S7ICIUWA225O5PSJWUBEMXGFSSW2PQFO6FP4DQEKMS5DASRGRR'
		),
		transactionHash: demoHash('82'.repeat(32)),
		deliveries: [
			{ channel: 'telegram', status: 'sent', sentAt: '2026-07-16T08:17:06.000Z' },
			{ channel: 'webhook', status: 'sent', sentAt: '2026-07-16T08:17:06.000Z' }
		]
	},
	{
		id: 'demo-alert-001',
		kind: 'wallet-activity',
		severity: 'low',
		title: 'Contract activity threshold reached',
		summary: 'The demo wallet crossed its configured hourly contract-transaction threshold.',
		detectedAt: '2026-07-04T19:02:15.000Z',
		ledgerSequence: 63_284_092,
		transactionHash: demoHash('19'.repeat(32)),
		deliveries: [
			{ channel: 'telegram', status: 'sent', sentAt: '2026-07-04T19:02:18.000Z' }
		]
	}
];

export const DEMO_WALLET_DASHBOARD: WalletDashboard = {
	source: DEMO_DATASET_METADATA,
	summary: {
		accountAddress: DEMO_WALLET_ADDRESS,
		firstInteractionAt: '2025-08-04T09:18:21.000Z',
		lastInteractionAt: '2026-07-25T17:42:08.000Z',
		transactionCount: 18_492,
		successfulTransactionCount: 18_291,
		protocolCount: DEMO_PROTOCOLS.length,
		contractCount: 11,
		alertCount: DEMO_ALERTS.length
	},
	protocols: DEMO_PROTOCOLS,
	alerts: DEMO_ALERTS
};

const DEMO_LEADERBOARD_BASE: readonly Omit<LeaderboardRow, 'rank'>[] = [
	{
		accountAddress: demoAccount('GAVDWTC5NZ7ZBINSYPKOL5QHDAUTUS24NV7I7IFRYLJ6J5IGC4UDS7OX'),
		transactionCount: 192_842,
		successfulTransactionCount: 190_106,
		protocolCount: 7,
		contractCount: 42,
		lastInteractionAt: '2026-07-25T17:59:51.000Z'
	},
	{
		accountAddress: demoAccount('GBEVU234RWPK7QGR4LZQIFJGG5EFS2T3RSO25P6Q4HZAGFBFGZDVQMKK'),
		transactionCount: 154_267,
		successfulTransactionCount: 152_881,
		protocolCount: 5,
		contractCount: 31,
		lastInteractionAt: '2026-07-25T17:57:04.000Z'
	},
	{
		accountAddress: demoAccount('GBUHTCU3VS645X7QAEJCGNCFKZTXRCM2VO6M3XXPAAISEM2EKVTHPEPS'),
		transactionCount: 98_031,
		successfulTransactionCount: 96_544,
		protocolCount: 8,
		contractCount: 56,
		lastInteractionAt: '2026-07-25T17:49:16.000Z'
	},
	{
		accountAddress: demoAccount('GCDZRKN2ZPOO37QPEAYUEU3EOWDJPKFZZLN6Z7IOD4YECUTDOSCZNPQH'),
		transactionCount: 72_416,
		successfulTransactionCount: 71_992,
		protocolCount: 6,
		contractCount: 28,
		lastInteractionAt: '2026-07-25T17:45:38.000Z'
	},
	{
		accountAddress: DEMO_WALLET_ADDRESS,
		transactionCount: 18_492,
		successfulTransactionCount: 18_291,
		protocolCount: 4,
		contractCount: 11,
		lastInteractionAt: '2026-07-25T17:42:08.000Z'
	},
	{
		accountAddress: demoAccount('GCTLPSGZ5L5QYHJOH5IGC4UDSSS3NR6Y5H5AWHBNHZHWA4MCSOSLKG4C'),
		transactionCount: 15_407,
		successfulTransactionCount: 15_122,
		protocolCount: 9,
		contractCount: 63,
		lastInteractionAt: '2026-07-25T17:39:40.000Z'
	},
	{
		accountAddress: demoAccount('GDC5NZ7YBENCWPCNLZXYBENCWPCNLZXXBAMSUO2MLVXH7EFBWLB5IJSQ'),
		transactionCount: 11_985,
		successfulTransactionCount: 11_641,
		protocolCount: 6,
		contractCount: 24,
		lastInteractionAt: '2026-07-25T17:31:24.000Z'
	},
	{
		accountAddress: demoAccount('GDSPKBQXFA4UUW3MPWHJ7MGB2LR7IBIWE44ESWTLPSGZ5L6A2HRPH44L'),
		transactionCount: 8_422,
		successfulTransactionCount: 8_173,
		protocolCount: 3,
		contractCount: 14,
		lastInteractionAt: '2026-07-25T17:26:09.000Z'
	}
];

export class DemoDashboardRepository implements DashboardRepository {
	readonly metadata = DEMO_DATASET_METADATA;

	async getWallet(address: Parameters<DashboardRepository['getWallet']>[0]): Promise<WalletDashboard | null> {
		const parsed = parseStellarWalletAddress(address);
		if (!parsed.ok || parsed.value.accountAddress !== DEMO_WALLET_ADDRESS) return null;
		return DEMO_WALLET_DASHBOARD;
	}

	async getLeaderboard({ metric, limit = 10 }: LeaderboardQuery): Promise<Leaderboard> {
		const safeLimit = Math.min(100, Math.max(1, Math.trunc(limit)));
		const metricKey = metric === 'transactions' ? 'transactionCount' : 'protocolCount';
		const rows = [...DEMO_LEADERBOARD_BASE]
			.sort(
				(left, right) =>
					right[metricKey] - left[metricKey] ||
					left.accountAddress.localeCompare(right.accountAddress)
			)
			.slice(0, safeLimit)
			.map((row, index): LeaderboardRow => ({ rank: index + 1, ...row }));
		return { source: this.metadata, metric, rows };
	}
}

export const demoDashboardRepository: DashboardRepository = new DemoDashboardRepository();

function demoAccount(value: string): StellarAccountAddress {
	const parsed = parseStellarWalletAddress(value);
	if (!parsed.ok || parsed.value.kind !== 'account') {
		throw new Error(`Invalid deterministic demo account fixture: ${value}`);
	}
	return parsed.value.address;
}

function demoMuxedAccount(value: string): StellarMuxedAddress {
	const parsed = parseStellarWalletAddress(value);
	if (!parsed.ok || parsed.value.kind !== 'muxed-account') {
		throw new Error(`Invalid deterministic demo muxed-account fixture: ${value}`);
	}
	return parsed.value.address;
}

function demoContract(value: string): StellarContractAddress {
	if (!/^C[A-Z2-7]{55}$/.test(value)) {
		throw new Error(`Invalid deterministic demo contract fixture: ${value}`);
	}
	return value as StellarContractAddress;
}

function demoHash(value: string): StellarTransactionHash {
	if (!/^[0-9a-f]{64}$/.test(value)) {
		throw new Error(`Invalid deterministic demo transaction hash fixture: ${value}`);
	}
	return value as StellarTransactionHash;
}
