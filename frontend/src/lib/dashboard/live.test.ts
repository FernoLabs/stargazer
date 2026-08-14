import assert from 'node:assert/strict';
import test from 'node:test';
import {
	LiveDashboardClient,
	LiveDashboardAbortedError,
	LiveDashboardHttpError,
	LiveDashboardResponseError,
	LiveDashboardTimeoutError,
	isLeaderboardUnavailable
} from './live';
import { compareStellarStrKeyOrder, parseStellarWalletAddress } from './stellar';

const ACCOUNT = 'GA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQHES5';
const CONTRACT = 'CD25MNVTZDL4Y3XBCPCJXGXATV5WUHHOWMYFF4YBEGU5FCPGMYTVG5JY';
const UNATTRIBUTED_CONTRACT =
	'CCCCIQSDILITHMM7PBSLVDT5MISSY7R26MNZXCX4H7J5JQ5FPIYOGYFS';
const RAW_ORDER_LETTER_CONTRACT =
	'CAAAETSKQINKUVCVJM5B2BVMVRIOTJLR77IKBR2GWA4FNL3WWZYUXYLI';
const RAW_ORDER_DIGIT_CONTRACT =
	'CAAAE6DTPSRMLUDJQ3R5BU2UJ4L6FI74MVGRPP44K4NEZY3Z7HWYRS43';
const YIELDBLOX_POOL = 'CCCCIQSDILITHMM7PBSLVDT5MISSY7R26MNZXCX4H7J5JQ5FPIYOGYFS';
const INCIDENT_TRANSACTION =
	'ae721cacee382bdecac8d2c47286ecd42cb4711f658bb2aec7cba60dc64a31ff';

const parsedAccount = parseStellarWalletAddress(ACCOUNT);
if (!parsedAccount.ok) throw new Error('Invalid live client test account');

const source = {
	mode: 'live',
	label: 'NAS compact index',
	indexedFrom: '2025-05-28T09:28:29.000Z',
	asOf: '2026-07-27T14:00:00.000Z',
	network: 'Stellar Mainnet',
	indexedLedgerStart: 57_280_000,
	indexedLedgerEnd: 63_615_999,
	indexedContractTransactionCount: 42_000_123,
	disclaimer: 'Exact contract relationships with registry attribution where available.'
};

function walletResponse(alerts: unknown, alertCount: number | null = null) {
	return {
		source,
		query: { inputAddress: ACCOUNT, accountAddress: ACCOUNT, muxedId: null },
		summary: {
			accountAddress: ACCOUNT,
			firstInteractionAt: '2025-08-04T09:18:21.000Z',
			lastInteractionAt: null,
			transactionCount: null,
			successfulTransactionCount: null,
			transactionRank: null,
			protocolRank: null,
			rankedWalletCount: null,
			protocolCount: 1,
			contractCount: 2,
			unattributedContractCount: 1,
			alertCount
		},
		protocols: [
			{
				protocolId: 'blend-v2',
				contractCount: 1,
				firstInteractionAt: '2025-08-04T09:18:21.000Z',
				firstInteractionLedger: 57_300_000,
				transactionCount: null,
				successfulTransactionCount: null,
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
					protocols: [{ protocolId: 'blend-v2', role: 'pool-factory', sourceUrl: null }]
				},
				{
					contractAddress: UNATTRIBUTED_CONTRACT,
					firstInteractionLedger: 57_300_100,
					firstInteractionAt: '2025-08-04T09:26:41.000Z',
					roleMask: 2,
					attributionStatus: 'unattributed',
					placeholderProtocolId: UNATTRIBUTED_CONTRACT,
					protocols: []
				}
			],
			limit: 100,
			nextCursor: null
		},
		alerts,
		capabilities: {
			walletContractRelationships: true,
			protocolAttribution: 'seed-roots-only',
			unattributedContractPlaceholders: true,
			walletTransactionCounts: false,
			walletLastActivity: false,
			alerts: alertCount !== null,
			historicalFindings: alertCount !== null,
			alertDelivery: false,
			leaderboard: false
		}
	};
}

test('maps exact wallet relationships while preserving unavailable values as null', async () => {
	let requestedUrl = '';
	const client = new LiveDashboardClient('http://nas.test/', {
		fetch: async (input) => {
			requestedUrl = String(input);
			return jsonResponse(
				walletResponse({
					available: false,
					reason: 'Historical finding rules are not configured.',
					deliveryAvailable: false,
					deliveryReason: 'Historical replay never attempts alert delivery.',
					coverage: null,
					items: []
				})
			);
		}
	});

	const result = await client.getWallet(parsedAccount.value.address);
	assert.equal(
		requestedUrl,
		`http://nas.test/api/wallets/${ACCOUNT}?limit=100`
	);
	assert.equal(result.summary.transactionCount, null);
	assert.equal(result.summary.transactionRank, null);
	assert.equal(result.summary.protocolRank, null);
	assert.equal(result.summary.rankedWalletCount, null);
	assert.equal(result.source.indexedContractTransactionCount, 42_000_123);
	assert.equal(result.protocols[0]?.protocolId, 'blend-v2');
	assert.equal(result.contracts.items[0]?.contractAddress, CONTRACT);
	assert.equal(result.summary.unattributedContractCount, 1);
	assert.equal(result.contracts.items[1]?.attributionStatus, 'unattributed');
	assert.equal(result.contracts.items[1]?.placeholderProtocolId, UNATTRIBUTED_CONTRACT);
	assert.deepEqual(result.contracts.items[1]?.protocols, []);
	assert.equal(result.alerts.available, false);
	assert.equal(result.alerts.coverage, null);
	assert.equal(result.alerts.deliveryAvailable, false);
});

test('maps exact wallet activity ranks from the finalized snapshot', async () => {
	const response = walletResponse({
		available: false,
		reason: 'Historical finding rules are not configured.',
		deliveryAvailable: false,
		deliveryReason: 'Historical replay never attempts alert delivery.',
		coverage: null,
		items: []
	});
	Object.assign(response.summary, {
		transactionCount: 3_225,
		successfulTransactionCount: 3_214,
		transactionRank: 12_578,
		protocolRank: 1,
		rankedWalletCount: 251_997
	});
	const client = new LiveDashboardClient('http://nas.test', {
		fetch: async () => jsonResponse(response)
	});

	const result = await client.getWallet(parsedAccount.value.address);
	assert.equal(result.summary.transactionRank, 12_578);
	assert.equal(result.summary.protocolRank, 1);
	assert.equal(result.summary.rankedWalletCount, 251_997);
});

test('rejects an unattributed placeholder that does not match its on-chain contract address', async () => {
	const response = walletResponse({
		available: false,
		reason: 'Historical finding rules are not configured.',
		deliveryAvailable: false,
		deliveryReason: 'Historical replay never attempts alert delivery.',
		coverage: null,
		items: []
	});
	(response.contracts.items[1] as { placeholderProtocolId: string }).placeholderProtocolId = CONTRACT;
	const client = new LiveDashboardClient('http://nas.test', {
		fetch: async () => jsonResponse(response)
	});

	await assert.rejects(client.getWallet(parsedAccount.value.address), (error: unknown) => {
		assert.ok(error instanceof LiveDashboardResponseError);
		assert.match(error.message, /placeholder must equal contractAddress/);
		return true;
	});
});

test('maps the contract-ID ordered unattributed index without inventing protocol names', async () => {
	let requestedUrl = '';
	const client = new LiveDashboardClient('http://nas.test', {
		fetch: async (input) => {
			requestedUrl = String(input);
			return jsonResponse({
				source,
				attribution: 'unattributed',
				items: [
					{
						contractAddress: UNATTRIBUTED_CONTRACT,
						attributionStatus: 'unattributed',
						placeholderProtocolId: UNATTRIBUTED_CONTRACT,
						protocols: [],
						firstLedger: 57_300_100,
						firstSeenAt: '2025-08-04T09:26:41.000Z',
						lastLedger: 63_000_000,
						lastSeenAt: '2026-06-10T09:18:21.000Z',
						transactionCount: 12,
						successfulTransactionCount: 11,
						currentExecutable: {
							ledger: 60_000_000,
							observedAt: '2026-01-01T00:00:00.000Z',
							executableKind: 'wasm',
							wasmHash: '11'.repeat(32),
							exact: true
						}
					}
				],
				limit: 1,
				nextCursor: UNATTRIBUTED_CONTRACT
			});
		}
	});

	const result = await client.getUnattributedContracts({ limit: 1 });
	assert.equal(requestedUrl, 'http://nas.test/api/contracts/unattributed?limit=1');
	assert.equal(result.attribution, 'unattributed');
	assert.equal(result.items[0]?.placeholderProtocolId, UNATTRIBUTED_CONTRACT);
	assert.deepEqual(result.items[0]?.protocols, []);
	assert.equal(result.items[0]?.transactionCount, 12);
	assert.equal(result.items[0]?.currentExecutable?.executableKind, 'wasm');
});

test('orders contract StrKeys with Stellar base32 digits after letters', () => {
	assert.ok(compareStellarStrKeyOrder(RAW_ORDER_LETTER_CONTRACT, RAW_ORDER_DIGIT_CONTRACT) < 0);
	assert.ok(compareStellarStrKeyOrder(RAW_ORDER_DIGIT_CONTRACT, RAW_ORDER_LETTER_CONTRACT) > 0);
});

test('maps a source-backed YieldBlox finding as historical replay without delivery', async () => {
	const finding = {
		id: 'yieldblox-2026-contract-relationship',
		kind: 'historical-security-incident',
		severity: 'critical',
		title: 'Wallet exposed to the YieldBlox incident pool',
		summary: 'The wallet interacted with the verified victim pool in the replay window.',
		incidentAt: '2026-02-22T00:22:09.000Z',
		ledgerSequence: 61_340_384,
		protocolId: 'yieldblox',
		protocolName: 'YieldBlox',
		contractAddress: YIELDBLOX_POOL,
		transactionHash: INCIDENT_TRANSACTION,
		origin: 'historical-replay',
		matchType: 'contract-relationship',
		sources: [
			{
				label: 'BlockSec forensic report',
				url: 'https://blocksec.com/blog/yieldblox-dao-incident-on-stellar-oracle-misconfiguration-enabled-a-10m-drain'
			}
		],
		deliveries: []
	};
	const client = new LiveDashboardClient('http://nas.test', {
		fetch: async () =>
			jsonResponse(
				walletResponse(
					{
						available: true,
						reason: 'YieldBlox incident replay completed.',
						deliveryAvailable: false,
						deliveryReason: 'Historical replay never attempts alert delivery.',
						coverage: { fromLedger: 61_339_940, toLedger: 61_340_408 },
						items: [finding]
					},
					1
				)
			)
	});

	const result = await client.getWallet(parsedAccount.value.address);
	assert.equal(result.alerts.available, true);
	assert.equal(result.alerts.coverage?.fromLedger, 61_339_940);
	assert.equal(result.alerts.items[0]?.origin, 'historical-replay');
	assert.equal(result.alerts.items[0]?.matchType, 'contract-relationship');
	assert.deepEqual(result.alerts.items[0]?.deliveries, []);
});

test('rejects delivery claims attached to a historical replay finding', async () => {
	const client = new LiveDashboardClient('http://nas.test', {
		fetch: async () =>
			jsonResponse(
				walletResponse(
					{
						available: true,
						reason: 'YieldBlox incident replay completed.',
						deliveryAvailable: false,
						deliveryReason: 'Historical replay never attempts alert delivery.',
						coverage: { fromLedger: 61_339_940, toLedger: 61_340_408 },
						items: [
							{
								id: 'invalid-delivery-claim',
								kind: 'historical-security-incident',
								severity: 'critical',
								title: 'Invalid historical delivery claim',
								summary: 'Historical replay cannot claim that an alert was sent.',
								incidentAt: '2026-02-22T00:22:09.000Z',
								ledgerSequence: 61_340_384,
								protocolId: 'yieldblox',
								protocolName: 'YieldBlox',
								contractAddress: YIELDBLOX_POOL,
								transactionHash: INCIDENT_TRANSACTION,
								origin: 'historical-replay',
								matchType: 'verified-incident-participant',
								sources: [{ label: 'Evidence', url: 'https://example.com/evidence' }],
								deliveries: [{ channel: 'telegram', status: 'sent' }]
							}
						]
					},
					1
				)
			)
	});

	await assert.rejects(client.getWallet(parsedAccount.value.address), (error: unknown) => {
		assert.ok(error instanceof LiveDashboardResponseError);
		assert.match(error.message, /deliveries must be empty/);
		return true;
	});
});

test('exposes leaderboard startup unavailability as a typed HTTP error', async () => {
	const client = new LiveDashboardClient('http://nas.test', {
		fetch: async () =>
			jsonResponse(
				{
					error: {
						code: 'leaderboard_unavailable',
						message: 'wallet_activity has not been published'
					}
				},
				503
			)
	});

	await assert.rejects(
		client.getLeaderboard({ metric: 'transactions' }),
		(error: unknown) => {
			assert.ok(error instanceof LiveDashboardHttpError);
			assert.equal(error.status, 503);
			assert.ok(isLeaderboardUnavailable(error));
			return true;
		}
	);
});

test('parses real example wallets without treating the sample as a ranking', async () => {
	const client = new LiveDashboardClient('http://nas.test', {
		fetch: async () =>
			jsonResponse({
				source,
				selection:
					'lexicographic sample from the exact wallet-contract relationship index; not a ranking',
				wallets: [
					{
						accountAddress: ACCOUNT,
						firstInteractionAt: '2025-08-04T09:18:21.000Z',
						firstInteractionLedger: 57_300_000
					}
				]
			})
	});

	const result = await client.getExampleWallets({ limit: 1 });
	assert.match(result.selection, /not a ranking/);
	assert.equal(result.wallets[0]?.accountAddress, ACCOUNT);
});

test('maps exact protocol versions, upgrades, and live governance policy alerts', async () => {
	let requestedUrl = '';
	const baselineHash = '11'.repeat(32);
	const updateHash = '22'.repeat(32);
	const transactionHash = '33'.repeat(32);
	const policyBefore = {
		thresholds: { low: 1, medium: 1, high: 1 },
		masterWeight: 1,
		signers: [{ key: ACCOUNT, type: 'ed25519_public_key', weight: 1 }],
		classification: 'single-key'
	};
	const policyAfter = {
		thresholds: { low: 1, medium: 1, high: 1 },
		masterWeight: 0,
		signers: [{ key: ACCOUNT, type: 'ed25519_public_key', weight: 0 }],
		classification: 'locked'
	};
	const client = new LiveDashboardClient('http://nas.test', {
		fetch: async (input) => {
			requestedUrl = String(input);
			return jsonResponse({
				source,
				protocols: [
					{
						protocolId: 'blend-v2',
						contractCount: 1,
						versionObservationCount: 2,
						upgradeCount: 1,
						contracts: [
							{
								contractAddress: CONTRACT,
								roles: [{ role: 'pool-factory', sourceUrl: null }],
								usage: {
									firstLedger: 57_300_000,
									firstSeenAt: '2025-08-04T09:18:21.000Z',
									lastLedger: 63_000_000,
									lastSeenAt: '2026-06-10T09:18:21.000Z',
									transactionCount: 12,
									successfulTransactionCount: 11
								},
								currentExecutable: {
									ledger: 60_000_000,
									observedAt: '2026-01-01T00:00:00.000Z',
									executableKind: 'wasm',
									wasmHash: updateHash,
									exact: true
								},
								versions: [
									{
										ledger: 57_300_000,
										observedAt: '2025-08-04T09:18:21.000Z',
										applyOrder: 1,
										transactionHash,
										executableKind: 'wasm',
										wasmHash: baselineHash,
										sourceKind: 'observed_state',
										activeBeforeWindow: true,
										exactUpgrade: false
									},
									{
										ledger: 60_000_000,
										observedAt: '2026-01-01T00:00:00.000Z',
										applyOrder: 2,
										transactionHash,
										executableKind: 'wasm',
										wasmHash: updateHash,
										sourceKind: 'updated',
										activeBeforeWindow: false,
										exactUpgrade: true
									}
								]
							}
						]
					}
				],
				alerts: [
					{
						alertId: 'upgrade-1',
						kind: 'contract_upgrade',
						severity: 'info',
						protocolId: 'blend-v2',
						contractAddress: CONTRACT,
						roles: ['pool-factory'],
						ledger: 60_000_000,
						observedAt: '2026-01-01T00:00:00.000Z',
						transactionHash,
						previousWasmHash: baselineHash,
						newWasmHash: updateHash
					}
				],
				governance: {
					available: true,
					reason: 'Complete Horizon controller cycle.',
					observedAt: '2026-07-27T19:31:33.000Z',
					stale: false,
					controllerCount: 1,
					controllers: [
						{
							accountId: ACCOUNT,
							protocol: 'Blend v2',
							role: 'pool-admin',
							sources: ['https://example.test/governance'],
							lastModifiedLedger: 63_600_000,
							policy: policyAfter
						}
					],
					alerts: [
						{
							id: '44'.repeat(32),
							type: 'classic-controller-policy-changed',
							severity: 'critical',
							detectedAt: '2026-07-27T19:31:33.000Z',
							accountId: ACCOUNT,
							protocol: 'Blend v2',
							role: 'pool-admin',
							beforeObservedAt: '2026-07-27T19:30:33.000Z',
							afterLastModifiedLedger: 63_600_000,
							before: policyBefore,
							after: policyAfter
						},
						{
							id: '55'.repeat(32),
							type: 'classic-controller-transaction-executed',
							severity: 'high',
							detectedAt: '2026-07-27T19:32:00.000Z',
							occurredAt: '2026-07-27T19:31:50.000Z',
							accountId: ACCOUNT,
							protocol: 'Blend v2',
							role: 'pool-admin',
							transactionHash,
							operations: [
								{
									id: '273796412704575582',
									type: 'manage_data',
									typeCode: 10,
									sourceAccount: ACCOUNT,
									payload: { name: 'emergency-mode', value: 'enabled' }
								}
							]
						}
					]
				},
				capabilities: {
					executableHistory: true,
					upgradeAlerts: true,
					multisigMonitoring: true,
					multisigReason: 'Complete Horizon controller cycles are monitored.'
				}
			});
		}
	});

	const result = await client.getProtocols();
	assert.equal(requestedUrl, 'http://nas.test/api/protocols');
	assert.equal(result.protocols[0]?.upgradeCount, 1);
	assert.equal(result.alerts[0]?.previousWasmHash, baselineHash);
	assert.equal(result.protocols[0]?.contracts[0]?.versions[1]?.exactUpgrade, true);
	assert.equal(result.capabilities.multisigMonitoring, true);
	assert.equal(result.governance.available, true);
	if (!result.governance.available) throw new Error('Expected live governance monitoring');
	assert.equal(result.governance.controllerCount, 1);
	assert.equal(result.governance.controllers[0]?.policy.classification, 'locked');
	const policyAlert = result.governance.alerts[0];
	assert.equal(
		policyAlert?.type === 'classic-controller-policy-changed'
			? policyAlert.before.classification
			: null,
		'single-key'
	);
	const activityAlert = result.governance.alerts[1];
	assert.equal(
		activityAlert?.type === 'classic-controller-transaction-executed'
			? activityAlert.operations[0]?.payload.name
			: null,
		'emergency-mode'
	);
});

test('rejects governance observations in an unavailable watcher state', async () => {
	const client = new LiveDashboardClient('http://nas.test', {
		fetch: async () =>
			jsonResponse({
				source,
				protocols: [],
				alerts: [],
				governance: {
					available: false,
					reason: 'No complete baseline.',
					observedAt: null,
					stale: null,
					controllerCount: 1,
					controllers: [],
					alerts: []
				},
				capabilities: {
					executableHistory: true,
					upgradeAlerts: true,
					multisigMonitoring: false,
					multisigReason: 'No complete baseline.'
				}
			})
	});

	await assert.rejects(
		client.getProtocols(),
		(error: unknown) => error instanceof LiveDashboardResponseError
	);
});

test('rejects a malformed successful response instead of casting it', async () => {
	const client = new LiveDashboardClient('http://nas.test', {
		fetch: async () => jsonResponse({ source: { mode: 'demo' } })
	});

	await assert.rejects(
		client.getLeaderboard({ metric: 'protocols' }),
		(error: unknown) => error instanceof LiveDashboardResponseError
	);
});

test('distinguishes request timeouts from ordinary network errors', async () => {
	const client = new LiveDashboardClient('http://nas.test', {
		timeoutMs: 5,
		fetch: (_input, init) =>
			new Promise<Response>((_resolve, reject) => {
				init?.signal?.addEventListener(
					'abort',
					() => reject(init.signal?.reason ?? new Error('aborted')),
					{ once: true }
				);
			})
	});

	await assert.rejects(
		client.getExampleWallets(),
		(error: unknown) => error instanceof LiveDashboardTimeoutError
	);
});

test('honors a caller AbortSignal before starting a request', async () => {
	const controller = new AbortController();
	controller.abort(new Error('route navigation'));
	let fetchCalled = false;
	const client = new LiveDashboardClient('http://nas.test', {
		fetch: async () => {
			fetchCalled = true;
			return jsonResponse({});
		}
	});

	await assert.rejects(
		client.getExampleWallets({ signal: controller.signal }),
		(error: unknown) => error instanceof LiveDashboardAbortedError
	);
	assert.equal(fetchCalled, false);
});

function jsonResponse(payload: unknown, status = 200): Response {
	return new Response(JSON.stringify(payload), {
		status,
		headers: { 'content-type': 'application/json' }
	});
}
