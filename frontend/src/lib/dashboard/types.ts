declare const stellarAccountAddressBrand: unique symbol;
declare const stellarMuxedAddressBrand: unique symbol;
declare const stellarContractAddressBrand: unique symbol;
declare const stellarTransactionHashBrand: unique symbol;

/** A checksum-validated Stellar G-address. */
export type StellarAccountAddress = string & {
	readonly [stellarAccountAddressBrand]: 'StellarAccountAddress';
};

/** A checksum-validated Stellar M-address. */
export type StellarMuxedAddress = string & {
	readonly [stellarMuxedAddressBrand]: 'StellarMuxedAddress';
};

export type StellarWalletAddress = StellarAccountAddress | StellarMuxedAddress;

export type StellarContractAddress = string & {
	readonly [stellarContractAddressBrand]: 'StellarContractAddress';
};

/** A lowercase, 64-character transaction hash. */
export type StellarTransactionHash = string & {
	readonly [stellarTransactionHashBrand]: 'StellarTransactionHash';
};

export type DashboardDataMode = 'demo' | 'live';

export interface DashboardDatasetMetadata {
	readonly mode: DashboardDataMode;
	readonly label: string;
	/** UTC close time of the first ledger included in the indexed archive window. */
	readonly indexedFrom?: string;
	/** UTC close time of the last ledger included in the indexed archive window. */
	readonly asOf: string;
	readonly network: 'Stellar Mainnet';
	readonly indexedLedgerStart?: number;
	readonly indexedLedgerEnd?: number;
	/**
	 * Exact count of transactions retained by the compact index because they
	 * interacted with at least one contract. This is not every transaction in
	 * the covered Stellar ledgers.
	 */
	readonly indexedContractTransactionCount?: number;
	readonly disclaimer?: string;
}

export interface WalletSummary {
	/** The G-address used as the compact index lookup key. */
	readonly accountAddress: StellarAccountAddress;
	readonly firstInteractionAt: string;
	readonly lastInteractionAt: string;
	readonly transactionCount: number;
	readonly successfulTransactionCount: number;
	readonly protocolCount: number;
	readonly contractCount: number;
	readonly alertCount: number;
}

export type ProtocolCategory =
	| 'lending'
	| 'exchange'
	| 'liquidity'
	| 'oracle'
	| 'vault'
	| 'payments'
	| 'other';

export type ProtocolAttributionConfidence = 'authoritative' | 'inferred' | 'unknown';

export interface ProtocolInteraction {
	readonly protocolId: string;
	readonly protocolName: string;
	readonly category: ProtocolCategory;
	readonly attributionConfidence: ProtocolAttributionConfidence;
	readonly transactionCount: number;
	readonly successfulTransactionCount: number;
	readonly contractCount: number;
	readonly firstInteractionAt: string;
	readonly lastInteractionAt: string;
	readonly lastTransactionHash?: StellarTransactionHash;
	readonly contracts: readonly StellarContractAddress[];
}

export type AlertSeverity = 'info' | 'low' | 'medium' | 'high' | 'critical';

export type OperationalAlertKind =
	| 'new-protocol-interaction'
	| 'contract-executable-change'
	| 'contract-availability-change'
	| 'wallet-activity'
	| 'dependency-change';

export type AlertKind = OperationalAlertKind | 'historical-security-incident';

export type AlertDeliveryChannel = 'email' | 'telegram' | 'webhook';
export type AlertDeliveryStatus = 'sent' | 'failed' | 'pending' | 'suppressed';

export interface AlertDelivery {
	readonly channel: AlertDeliveryChannel;
	readonly status: AlertDeliveryStatus;
	readonly sentAt?: string;
}

interface WalletAlertBase {
	readonly id: string;
	readonly severity: AlertSeverity;
	readonly title: string;
	readonly summary: string;
}

export interface OperationalWalletAlert extends WalletAlertBase {
	readonly kind: OperationalAlertKind;
	readonly detectedAt: string;
	readonly origin?: never;
	readonly matchType?: never;
	readonly sources?: never;
	readonly ledgerSequence?: number;
	readonly protocolId?: string;
	readonly protocolName?: string;
	readonly contractAddress?: StellarContractAddress;
	readonly transactionHash?: StellarTransactionHash;
	readonly deliveries: readonly AlertDelivery[];
}

export type HistoricalIncidentMatchType =
	| 'contract-relationship'
	| 'verified-incident-participant';

export interface HistoricalFindingSource {
	readonly label: string;
	readonly url: string;
}

/** A retrospective archive match. It was not delivered as a live alert. */
export interface HistoricalSecurityFinding extends WalletAlertBase {
	readonly kind: 'historical-security-incident';
	readonly incidentAt: string;
	readonly ledgerSequence: number;
	readonly protocolId: string;
	readonly protocolName: string;
	readonly contractAddress: StellarContractAddress;
	readonly transactionHash: StellarTransactionHash;
	readonly origin: 'historical-replay';
	readonly matchType: HistoricalIncidentMatchType;
	readonly sources: readonly HistoricalFindingSource[];
	readonly deliveries: readonly [];
}

export type WalletAlert = OperationalWalletAlert | HistoricalSecurityFinding;

export interface WalletDashboard {
	readonly source: DashboardDatasetMetadata;
	readonly summary: WalletSummary;
	readonly protocols: readonly ProtocolInteraction[];
	readonly alerts: readonly WalletAlert[];
}

export type LeaderboardMetric = 'transactions' | 'protocols';

export interface LeaderboardRow {
	readonly rank: number;
	readonly accountAddress: StellarAccountAddress;
	readonly transactionCount: number;
	readonly successfulTransactionCount: number;
	readonly protocolCount: number;
	readonly contractCount: number;
	readonly lastInteractionAt: string;
}

export interface Leaderboard {
	readonly source: DashboardDatasetMetadata;
	readonly metric: LeaderboardMetric;
	readonly rows: readonly LeaderboardRow[];
}
