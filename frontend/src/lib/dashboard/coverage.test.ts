import assert from 'node:assert/strict';
import test from 'node:test';
import { buildWalletAlertReport, buildWalletCoverageReport } from './coverage';
import type { LiveWalletResponse } from './live';
import type { OperationalWalletAlert } from './types';

const ACCOUNT = 'GA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQHES5';
const CONTRACT = 'CD25MNVTZDL4Y3XBCPCJXGXATV5WUHHOWMYFF4YBEGU5FCPGMYTVG5JY';
const UNATTRIBUTED = 'CCCCIQSDILITHMM7PBSLVDT5MISSY7R26MNZXCX4H7J5JQ5FPIYOGYFS';
const TRANSACTION = 'ae721cacee382bdecac8d2c47286ecd42cb4711f658bb2aec7cba60dc64a31ff';

function walletResponse({ finding = false, alertsAvailable = true } = {}): LiveWalletResponse {
	return {
		source: {
			mode: 'live',
			label: 'NAS compact index',
			indexedFrom: '2025-05-28T09:28:29.000Z',
			asOf: '2026-07-27T14:00:00.000Z',
			network: 'Stellar Mainnet',
			indexedLedgerStart: 57_280_000,
			indexedLedgerEnd: 63_615_999,
			indexedContractTransactionCount: 42_000_123
		},
		query: {
			inputAddress: ACCOUNT,
			accountAddress: ACCOUNT,
			muxedId: null
		},
		summary: {
			accountAddress: ACCOUNT,
			firstInteractionAt: '2025-08-04T09:18:21.000Z',
			lastInteractionAt: '2026-02-22T00:22:09.000Z',
			transactionCount: 42,
			successfulTransactionCount: 40,
			transactionRank: 120,
			protocolRank: 18,
			rankedWalletCount: 251_997,
			protocolCount: 1,
			contractCount: 2,
			unattributedContractCount: 1,
			alertCount: alertsAvailable ? (finding ? 1 : 0) : null
		},
		protocols: [
			{
				protocolId: 'blend-v2',
				contractCount: 1,
				firstInteractionAt: '2025-08-04T09:18:21.000Z',
				firstInteractionLedger: 57_300_000,
				transactionCount: 20,
				successfulTransactionCount: 20,
				contracts: [CONTRACT]
			}
		],
		contracts: {
			items: [
				{
					contractAddress: CONTRACT,
					firstInteractionLedger: 57_300_000,
					firstInteractionAt: '2025-08-04T09:18:21.000Z',
					roleMask: 1,
					attributionStatus: 'verified-registry',
					placeholderProtocolId: null,
					protocols: [{ protocolId: 'blend-v2', role: 'pool', sourceUrl: null }]
				},
				{
					contractAddress: UNATTRIBUTED,
					firstInteractionLedger: 57_300_100,
					firstInteractionAt: '2025-08-04T09:26:41.000Z',
					roleMask: 2,
					attributionStatus: 'unattributed',
					placeholderProtocolId: UNATTRIBUTED,
					protocols: []
				}
			],
			limit: 100,
			nextCursor: null
		},
		alerts: alertsAvailable
			? {
					available: true,
					reason: 'YieldBlox incident replay completed.',
					deliveryAvailable: false,
					deliveryReason: 'Historical replay never attempts alert delivery.',
					coverage: { fromLedger: 61_339_940, toLedger: 61_340_408 },
					items: finding
						? [
								{
									id: 'yieldblox-2026-contract-relationship',
									kind: 'historical-security-incident',
									severity: 'critical',
									title: 'Wallet exposed to the YieldBlox incident pool',
									summary: 'The wallet interacted with the verified victim pool.',
									incidentAt: '2026-02-22T00:22:09.000Z',
									ledgerSequence: 61_340_384,
									protocolId: 'yieldblox',
									protocolName: 'YieldBlox',
									contractAddress: UNATTRIBUTED,
									transactionHash: TRANSACTION,
									origin: 'historical-replay',
									matchType: 'contract-relationship',
									sources: [{ label: 'Forensic report', url: 'https://example.com/report' }],
									deliveries: []
								}
							]
						: []
				}
			: {
					available: false,
					reason: 'Historical finding rules are not configured.',
					deliveryAvailable: false,
					deliveryReason: 'Live delivery is not configured.',
					coverage: null,
					items: []
				},
		capabilities: {
			walletContractRelationships: true,
			protocolAttribution: 'seed-roots-only',
			unattributedContractPlaceholders: true,
			walletTransactionCounts: true,
			walletLastActivity: true,
			alerts: alertsAvailable,
			historicalFindings: alertsAvailable,
			alertDelivery: false,
			leaderboard: true
		}
	} as unknown as LiveWalletResponse;
}

test('reports exact coverage facts without inventing an ecosystem percentage', () => {
	const report = buildWalletCoverageReport(walletResponse());

	assert.deepEqual(report.facts, {
		transactionCount: 42,
		contractCount: 2,
		attributedContractCount: 1,
		unattributedContractCount: 1,
		protocolCount: 1
	});
	assert.equal(report.items.find((item) => item.id === 'wallet-contract-relationships')?.status, 'covered');
	assert.equal(report.items.find((item) => item.id === 'protocol-attribution')?.status, 'partial');
	assert.equal(report.items.find((item) => item.id === 'contract-upgrades')?.status, 'planned');
	assert.equal(report.items.find((item) => item.id === 'governance-changes')?.status, 'planned');
	assert.equal(report.items.find((item) => item.id === 'live-delivery')?.status, 'planned');
	assert.doesNotMatch(JSON.stringify(report), /percent|%/i);
});

test('keeps unavailable historical checks explicit instead of treating them as zero findings', () => {
	const wallet = walletResponse({ alertsAvailable: false });
	const coverage = buildWalletCoverageReport(wallet);
	const alerts = buildWalletAlertReport(wallet);

	assert.equal(coverage.items.find((item) => item.id === 'historical-incidents')?.status, 'planned');
	assert.equal(alerts.historicalCheck.available, false);
	assert.match(alerts.historicalCheck.summary, /No conclusion is made/);
	assert.equal(alerts.critical.length, 0);
});

test('classifies a YieldBlox replay match as critical retrospective evidence', () => {
	const report = buildWalletAlertReport(walletResponse({ finding: true }));

	assert.equal(report.critical.length, 1);
	assert.equal(report.critical[0]?.kind, 'historical-evidence');
	assert.equal(report.critical[0]?.severity, 'critical');
	assert.match(report.critical[0]?.summary ?? '', /no alert was sent/i);
	assert.equal(report.critical[0]?.sources[0]?.label, 'Forensic report');
	assert.ok(report.important.some((signal) => signal.kind === 'monitoring-scope'));
	assert.ok(report.important.some((signal) => signal.kind === 'coverage-gap'));
	assert.equal(report.delivery.available, false);
});

test('keeps a checked replay with no match distinct from a safety guarantee', () => {
	const report = buildWalletAlertReport(walletResponse());

	assert.equal(report.historicalCheck.available, true);
	assert.match(report.historicalCheck.summary, /0 retrospective matches returned/);
	assert.equal(report.critical.length, 0);
});

test('accepts current alerts, preserves severity grouping, and deduplicates IDs', () => {
	const importantAlert = {
		id: 'upgrade-observed',
		kind: 'contract-executable-change',
		severity: 'medium',
		title: 'Contract executable changed',
		summary: 'A monitored contract executable changed.',
		detectedAt: '2026-07-27T14:01:00.000Z',
		protocolId: 'blend-v2',
		protocolName: 'Blend v2',
		contractAddress: CONTRACT,
		transactionHash: TRANSACTION,
		deliveries: []
	} as unknown as OperationalWalletAlert;
	const report = buildWalletAlertReport(walletResponse({ finding: true }), [
		importantAlert,
		importantAlert
	]);

	assert.equal(report.important.filter((signal) => signal.id === importantAlert.id).length, 1);
	assert.equal(
		report.important.find((signal) => signal.id === importantAlert.id)?.kind,
		'current-alert'
	);
});
