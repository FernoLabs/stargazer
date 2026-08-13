import type { LiveWalletResponse } from './live';
import { protocolDisplayName } from './protocols';
import type {
	AlertSeverity,
	HistoricalFindingSource,
	WalletAlert
} from './types';

export type WalletCoverageStatus = 'covered' | 'partial' | 'planned';

export type WalletCoverageItemId =
	| 'wallet-contract-relationships'
	| 'protocol-attribution'
	| 'historical-incidents'
	| 'contract-upgrades'
	| 'governance-changes'
	| 'live-delivery';

export interface WalletCoverageItem {
	readonly id: WalletCoverageItemId;
	readonly label: string;
	readonly status: WalletCoverageStatus;
	readonly detail: string;
}

export interface WalletCoverageFacts {
	readonly transactionCount: number | null;
	readonly contractCount: number;
	readonly attributedContractCount: number;
	readonly unattributedContractCount: number;
	readonly protocolCount: number;
}

export interface WalletCoverageReportModel {
	readonly sourceLabel: string;
	readonly sourceIndexedFrom: string;
	readonly sourceAsOf: string;
	readonly indexedLedgerStart: number | null;
	readonly indexedLedgerEnd: number | null;
	readonly facts: WalletCoverageFacts;
	readonly items: readonly WalletCoverageItem[];
}

export type WalletAlertReportGroup = 'critical' | 'important';

export type WalletAlertReportSignalKind =
	| 'historical-evidence'
	| 'current-alert'
	| 'monitoring-scope'
	| 'coverage-gap';

export interface WalletAlertReportSignal {
	readonly id: string;
	readonly group: WalletAlertReportGroup;
	readonly kind: WalletAlertReportSignalKind;
	readonly severity: AlertSeverity;
	readonly title: string;
	readonly summary: string;
	readonly protocolId?: string;
	readonly protocolName?: string;
	readonly occurredAt?: string;
	readonly sources: readonly HistoricalFindingSource[];
}

export interface WalletAlertCheckState {
	readonly available: boolean;
	readonly summary: string;
}

export interface WalletAlertDeliveryState {
	readonly available: boolean;
	readonly summary: string;
}

export interface WalletAlertReportModel {
	readonly critical: readonly WalletAlertReportSignal[];
	readonly important: readonly WalletAlertReportSignal[];
	readonly historicalCheck: WalletAlertCheckState;
	readonly delivery: WalletAlertDeliveryState;
}

/**
 * Turns the wallet endpoint's declared capabilities into human-readable facts.
 * It intentionally has no percentage or inferred notion of total ecosystem coverage.
 */
export function buildWalletCoverageReport(wallet: LiveWalletResponse): WalletCoverageReportModel {
	const attributedContractCount = Math.max(
		0,
		wallet.summary.contractCount - wallet.summary.unattributedContractCount
	);
	const hasLiveDelivery =
		wallet.capabilities.alertDelivery && wallet.alerts.deliveryAvailable;

	return {
		sourceLabel: wallet.source.label,
		sourceIndexedFrom: wallet.source.indexedFrom,
		sourceAsOf: wallet.source.asOf,
		indexedLedgerStart: wallet.source.indexedLedgerStart ?? null,
		indexedLedgerEnd: wallet.source.indexedLedgerEnd ?? null,
		facts: {
			transactionCount: wallet.summary.transactionCount,
			contractCount: wallet.summary.contractCount,
			attributedContractCount,
			unattributedContractCount: wallet.summary.unattributedContractCount,
			protocolCount: wallet.summary.protocolCount
		},
		items: [
			{
				id: 'wallet-contract-relationships',
				label: 'Wallet and contract history',
				status: wallet.capabilities.walletContractRelationships ? 'covered' : 'planned',
				detail: wallet.capabilities.walletContractRelationships
					? relationshipDetail(wallet.summary.contractCount)
					: 'Exact wallet and contract relationships are not available in this response.'
			},
			{
				id: 'protocol-attribution',
				label: 'Protocol identification',
				status: 'partial',
				detail: attributionDetail(
					attributedContractCount,
					wallet.summary.unattributedContractCount,
					wallet.summary.protocolCount
				)
			},
			{
				id: 'historical-incidents',
				label: 'Historical incident checks',
				status: wallet.alerts.available ? 'partial' : 'planned',
				detail: historicalCoverageDetail(wallet)
			},
			{
				id: 'contract-upgrades',
				label: 'Contract upgrade alerts',
				status: 'planned',
				detail: 'Wallet-scoped upgrade monitoring is not declared by this wallet response.'
			},
			{
				id: 'governance-changes',
				label: 'Governance and multisig alerts',
				status: 'planned',
				detail: 'Wallet-scoped governance monitoring is not declared by this wallet response.'
			},
			{
				id: 'live-delivery',
				label: 'Live alert delivery',
				status: hasLiveDelivery ? 'covered' : 'planned',
				detail: hasLiveDelivery
					? wallet.alerts.deliveryReason
					: wallet.alerts.deliveryReason || 'Live alert delivery is not available.'
			}
		]
	};
}

/**
 * Separates evidence and current alerts by their declared severity, then adds
 * factual monitoring candidates from exact relationships. Candidates are never
 * represented as events that already happened.
 */
export function buildWalletAlertReport(
	wallet: LiveWalletResponse,
	currentAlerts: readonly WalletAlert[] = []
): WalletAlertReportModel {
	const seenAlertIds = new Set<string>();
	const signals: WalletAlertReportSignal[] = [];

	for (const alert of [...wallet.alerts.items, ...currentAlerts]) {
		if (seenAlertIds.has(alert.id)) continue;
		seenAlertIds.add(alert.id);
		signals.push(signalFromAlert(alert));
	}

	if (wallet.summary.protocolCount > 0) {
		const protocolNames = wallet.protocols
			.map((protocol) => protocolDisplayName(protocol.protocolId))
			.slice(0, 3);
		const remainingProtocolCount = Math.max(0, wallet.summary.protocolCount - protocolNames.length);
		const namedProtocols = [
			...protocolNames,
			...(remainingProtocolCount > 0 ? [`${remainingProtocolCount} more`] : [])
		].join(', ');
		signals.push({
			id: 'monitoring-scope:verified-protocols',
			group: 'important',
			kind: 'monitoring-scope',
			severity: 'info',
			title: `${wallet.summary.protocolCount} named protocol ${plural(
				wallet.summary.protocolCount,
				'dependency',
				'dependencies'
			)} identified`,
			summary: `${namedProtocols}. These exact relationships define a monitoring scope; this is not a detected protocol change.`,
			sources: []
		});
	}

	if (wallet.summary.unattributedContractCount > 0) {
		signals.push({
			id: 'coverage-gap:unattributed-contracts',
			group: 'important',
			kind: 'coverage-gap',
			severity: 'info',
			title: `${wallet.summary.unattributedContractCount} ${plural(
				wallet.summary.unattributedContractCount,
				'contract is',
				'contracts are'
			)} not attributed`,
			summary:
				'The relationships remain indexed by on-chain address, but cannot yet be grouped under a verified protocol name.',
			sources: []
		});
	}

	return {
		critical: signals.filter((signal) => signal.group === 'critical'),
		important: signals.filter((signal) => signal.group === 'important'),
		historicalCheck: {
			available: wallet.alerts.available,
			summary: wallet.alerts.available
				? historicalCheckSummary(wallet)
				: `${wallet.alerts.reason} No conclusion is made about untested incidents.`
		},
		delivery: {
			available:
				wallet.capabilities.alertDelivery && wallet.alerts.deliveryAvailable,
			summary: wallet.alerts.deliveryReason
		}
	};
}

function relationshipDetail(contractCount: number): string {
	if (contractCount === 0) {
		return 'No wallet and contract relationship was found in the indexed ledger range.';
	}
	return `${contractCount} exact wallet and contract ${plural(
		contractCount,
		'relationship',
		'relationships'
	)} found in the indexed ledger range.`;
}

function attributionDetail(
	attributedContractCount: number,
	unattributedContractCount: number,
	protocolCount: number
): string {
	if (attributedContractCount === 0 && unattributedContractCount === 0) {
		return 'Registry attribution is limited to verified seed roots; there are no wallet contracts to name.';
	}
	return `${attributedContractCount} ${plural(
		attributedContractCount,
		'contract has',
		'contracts have'
	)} verified registry attribution across ${protocolCount} named ${plural(
		protocolCount,
		'protocol',
		'protocols'
	)}; ${unattributedContractCount} remain address-only.`;
}

function historicalCoverageDetail(wallet: LiveWalletResponse): string {
	if (!wallet.alerts.available) {
		return `${wallet.alerts.reason} No historical incident result is inferred.`;
	}
	return `${historicalCheckSummary(wallet)} This is configured-rule coverage, not an exhaustive incident audit.`;
}

function historicalCheckSummary(wallet: LiveWalletResponse): string {
	const range = wallet.alerts.coverage;
	const rangeText = range
		? `Ledgers ${range.fromLedger}–${range.toLedger} were checked`
		: 'The configured replay was checked';
	return `${rangeText}; ${wallet.alerts.items.length} retrospective ${plural(
		wallet.alerts.items.length,
		'match',
		'matches'
	)} returned.`;
}

function signalFromAlert(alert: WalletAlert): WalletAlertReportSignal {
	if (alert.kind === 'historical-security-incident') {
		const forceCriticalYieldBlox =
			alert.protocolId === 'yieldblox' || alert.protocolId.startsWith('yieldblox-');
		return {
			id: alert.id,
			group: alert.severity === 'critical' || forceCriticalYieldBlox ? 'critical' : 'important',
			kind: 'historical-evidence',
			severity: forceCriticalYieldBlox ? 'critical' : alert.severity,
			title: alert.title,
			summary: `${alert.summary} Retrospective evidence only; no alert was sent at incident time.`,
			protocolId: alert.protocolId,
			protocolName: alert.protocolName,
			occurredAt: alert.incidentAt,
			sources: alert.sources
		};
	}

	return {
		id: alert.id,
		group: alert.severity === 'critical' ? 'critical' : 'important',
		kind: 'current-alert',
		severity: alert.severity,
		title: alert.title,
		summary: alert.summary,
		protocolId: alert.protocolId,
		protocolName: alert.protocolName,
		occurredAt: alert.detectedAt,
		sources: []
	};
}

function plural(count: number, singular: string, pluralValue: string): string {
	return count === 1 ? singular : pluralValue;
}
