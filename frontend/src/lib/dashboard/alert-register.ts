import { VERIFIED_INCIDENTS } from './intelligence';
import type {
	LiveGovernanceOperationPayload,
	LiveGovernancePolicy,
	LiveProtocolCatalogResponse
} from './live';
import { protocolDisplayName } from './protocols';

export type AlertRegisterCategory = 'incident' | 'contract-upgrade' | 'control-change';
export type AlertRegisterSeverity = 'critical' | 'high' | 'info';
export type AlertRegisterProvenance =
	| 'verified-intelligence'
	| 'archive-index'
	| 'forward-watcher';

export interface AlertRegisterSource {
	readonly label: string;
	readonly url: string;
}

export interface AlertRegisterRecord {
	readonly id: string;
	readonly category: AlertRegisterCategory;
	readonly severity: AlertRegisterSeverity;
	readonly provenance: AlertRegisterProvenance;
	readonly recordedAt: string | null;
	readonly timestampLabel: 'Occurred' | 'Observed' | 'Detected';
	readonly title: string;
	readonly summary: string;
	readonly context: string | null;
	readonly protocolId: string | null;
	readonly protocols: readonly string[];
	readonly ledger: number | null;
	readonly address: string | null;
	readonly transactionHash: string | null;
	readonly sources: readonly AlertRegisterSource[];
	readonly reportWallet: string | null;
}

const YIELDBLOX_REPORT_WALLET = 'GBO7VUL2TOKPWFAWKATIW7K3QYA7WQ63VDY5CAE6AFUUX6BHZBOC2WXC';

export function buildAlertRegister(
	catalog: LiveProtocolCatalogResponse | null
): readonly AlertRegisterRecord[] {
	const records: AlertRegisterRecord[] = VERIFIED_INCIDENTS.map((incident) => ({
		id: `incident:${incident.id}`,
		category: 'incident',
		severity: incident.severity,
		provenance: 'verified-intelligence',
		recordedAt: incident.occurredAt,
		timestampLabel: 'Occurred',
		title: incident.title,
		summary: incident.summary,
		context: incident.impact,
		protocolId: incident.id.startsWith('yieldblox-') ? 'yieldblox' : null,
		protocols: incident.protocols,
		ledger: incident.ledger,
		address: incident.primaryContract,
		transactionHash: incident.evidenceTransaction,
		sources: incident.sources,
		reportWallet: incident.id.startsWith('yieldblox-') ? YIELDBLOX_REPORT_WALLET : null
	}));

	if (catalog) {
		for (const alert of catalog.alerts) {
			const protocol = protocolDisplayName(alert.protocolId);
			const role = alert.roles.length > 0 ? ` (${alert.roles.join(', ')})` : '';
			records.push({
				id: `upgrade:${alert.alertId}`,
				category: 'contract-upgrade',
				severity: alert.severity,
				provenance: 'archive-index',
				recordedAt: alert.observedAt,
				timestampLabel: 'Observed',
				title: `${protocol} executable changed`,
				summary: `An exact executable transition was recorded for a registered protocol root${role}.`,
				context: `${shortHash(alert.previousWasmHash)} → ${shortHash(alert.newWasmHash)}`,
				protocolId: alert.protocolId,
				protocols: [protocol],
				ledger: alert.ledger,
				address: alert.contractAddress,
				transactionHash: alert.transactionHash,
				sources: [],
				reportWallet: null
			});
		}

		if (catalog.governance.available) {
			for (const alert of catalog.governance.alerts) {
				if (alert.type === 'classic-controller-transaction-executed') {
					const operationTypes = [...new Set(alert.operations.map((operation) => operation.type))];
					records.push({
						id: `control:${alert.id}`,
						category: 'control-change',
						severity: alert.severity,
						provenance: 'forward-watcher',
						recordedAt: alert.occurredAt,
						timestampLabel: 'Observed',
						title: `${alert.protocol} controller executed ${operationTypes.join(', ')}`,
						summary: `${alert.operations.length} successful on-chain operation${alert.operations.length === 1 ? '' : 's'} originated from the tracked ${alert.role} controller.`,
						context: operationPayloadLabel(alert.operations),
						protocolId: null,
						protocols: [alert.protocol],
						ledger: null,
						address: alert.accountId,
						transactionHash: alert.transactionHash,
						sources: [],
						reportWallet: null
					});
					continue;
				}
				records.push({
					id: `control:${alert.id}`,
					category: 'control-change',
					severity: alert.severity,
					provenance: 'forward-watcher',
					recordedAt: alert.detectedAt,
					timestampLabel: 'Detected',
					title: `${alert.protocol} controller policy changed`,
					summary: `The signer or threshold policy changed for the tracked ${alert.role} controller.`,
					context: `${policyLabel(alert.before)} → ${policyLabel(alert.after)}`,
					protocolId: null,
					protocols: [alert.protocol],
					ledger: alert.afterLastModifiedLedger,
					address: alert.accountId,
					transactionHash: null,
					sources: [],
					reportWallet: null
				});
			}
		}
	}

	return records.sort(compareNewestFirst);
}

function operationPayloadLabel(
	operations: readonly LiveGovernanceOperationPayload[]
): string {
	return operations
		.map((operation) => {
			const payload = JSON.stringify(operation.payload);
			const boundedPayload = payload.length > 240 ? `${payload.slice(0, 239)}…` : payload;
			return `${operation.type} (type ${operation.typeCode}) ${boundedPayload}`;
		})
		.join(' · ');
}

function compareNewestFirst(left: AlertRegisterRecord, right: AlertRegisterRecord): number {
	const leftTime = left.recordedAt ? Date.parse(left.recordedAt) : Number.NEGATIVE_INFINITY;
	const rightTime = right.recordedAt ? Date.parse(right.recordedAt) : Number.NEGATIVE_INFINITY;
	if (leftTime !== rightTime) return rightTime - leftTime;
	return left.id.localeCompare(right.id);
}

function policyLabel(policy: LiveGovernancePolicy): string {
	const activeSigners = policy.signers.filter((signer) => signer.weight > 0).length;
	return `${policy.classification}; thresholds ${policy.thresholds.low}/${policy.thresholds.medium}/${policy.thresholds.high}; ${activeSigners} active signer${activeSigners === 1 ? '' : 's'}`;
}

function shortHash(value: string | null): string {
	return value ? `${value.slice(0, 10)}…${value.slice(-8)}` : 'No Wasm hash';
}
