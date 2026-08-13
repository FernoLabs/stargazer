import { compareStellarStrKeyOrder, parseStellarWalletAddress } from './stellar';
import type {
	AlertSeverity,
	DashboardDatasetMetadata,
	HistoricalSecurityFinding,
	LeaderboardMetric,
	StellarAccountAddress,
	StellarContractAddress,
	StellarTransactionHash,
	StellarWalletAddress
} from './types';

const DEFAULT_TIMEOUT_MS = 15_000;
const DEFAULT_WALLET_CONTRACT_LIMIT = 100;
const MAX_WALLET_CONTRACT_LIMIT = 500;
const DEFAULT_CONTRACT_INDEX_LIMIT = 100;
const MAX_CONTRACT_INDEX_LIMIT = 500;
const DEFAULT_LEADERBOARD_LIMIT = 50;
const MAX_LEADERBOARD_LIMIT = 500;
const DEFAULT_EXAMPLE_WALLET_LIMIT = 5;
const MAX_EXAMPLE_WALLET_LIMIT = 20;

export interface LiveDashboardSource extends DashboardDatasetMetadata {
	readonly mode: 'live';
	readonly indexedFrom: string;
	readonly indexedContractTransactionCount: number;
}

/** Facts about the requested address and the base G-address used by the index. */
export interface LiveWalletQueryIdentity {
	readonly inputAddress: string;
	readonly accountAddress: StellarAccountAddress;
	readonly muxedId: string | null;
}

/**
 * Nullable fields are deliberately unavailable in the compact index until the
 * post-backfill wallet activity table has been published. Null never means 0.
 */
export interface LiveWalletSummary {
	readonly accountAddress: StellarAccountAddress;
	readonly firstInteractionAt: string | null;
	readonly lastInteractionAt: string | null;
	readonly transactionCount: number | null;
	readonly successfulTransactionCount: number | null;
	readonly transactionRank: number | null;
	readonly protocolRank: number | null;
	readonly rankedWalletCount: number | null;
	readonly protocolCount: number;
	readonly contractCount: number;
	readonly unattributedContractCount: number;
	readonly alertCount: number | null;
}

export interface LiveProtocolRelationship {
	readonly protocolId: string;
	readonly contractCount: number;
	readonly firstInteractionAt: string | null;
	readonly firstInteractionLedger: number;
	readonly transactionCount: number | null;
	readonly successfulTransactionCount: number | null;
	readonly contracts: readonly StellarContractAddress[];
}

export interface LiveContractProtocolRelationship {
	readonly protocolId: string;
	readonly role: string;
	readonly sourceUrl: string | null;
}

export type LiveContractAttributionStatus = 'verified-registry' | 'unattributed';

interface LiveContractRelationshipBase {
	readonly contractAddress: StellarContractAddress;
	readonly firstInteractionLedger: number;
	readonly firstInteractionAt: string | null;
	readonly roleMask: number;
	readonly protocols: readonly LiveContractProtocolRelationship[];
}

export interface LiveVerifiedContractRelationship extends LiveContractRelationshipBase {
	readonly attributionStatus: 'verified-registry';
	readonly placeholderProtocolId: null;
	readonly protocols: readonly LiveContractProtocolRelationship[];
}

export interface LiveUnattributedContractRelationship extends LiveContractRelationshipBase {
	readonly attributionStatus: 'unattributed';
	/** The on-chain address used as a UI identity until a source-backed name is known. */
	readonly placeholderProtocolId: StellarContractAddress;
	readonly protocols: readonly [];
}

export type LiveContractRelationship =
	| LiveVerifiedContractRelationship
	| LiveUnattributedContractRelationship;

export interface LiveContractPage {
	readonly items: readonly LiveContractRelationship[];
	readonly limit: number;
	readonly nextCursor: StellarContractAddress | null;
}

export type LiveContractIndexAttribution = 'all' | 'unattributed';

interface LiveIndexedContractBase {
	readonly contractAddress: StellarContractAddress;
	readonly firstLedger: number;
	readonly firstSeenAt: string | null;
	readonly lastLedger: number;
	readonly lastSeenAt: string | null;
	readonly transactionCount: number;
	readonly successfulTransactionCount: number;
	readonly currentExecutable: LiveCurrentContractExecutable | null;
}

export interface LiveVerifiedIndexedContract extends LiveIndexedContractBase {
	readonly attributionStatus: 'verified-registry';
	readonly placeholderProtocolId: null;
	readonly protocols: readonly LiveContractProtocolRelationship[];
}

export interface LiveUnattributedIndexedContract extends LiveIndexedContractBase {
	readonly attributionStatus: 'unattributed';
	readonly placeholderProtocolId: StellarContractAddress;
	readonly protocols: readonly [];
}

export type LiveIndexedContract = LiveVerifiedIndexedContract | LiveUnattributedIndexedContract;

export interface LiveContractIndexResponse {
	readonly source: LiveDashboardSource;
	readonly attribution: LiveContractIndexAttribution;
	readonly items: readonly LiveIndexedContract[];
	readonly limit: number;
	readonly nextCursor: StellarContractAddress | null;
}

export interface LiveUnattributedContractIndexResponse extends LiveContractIndexResponse {
	readonly attribution: 'unattributed';
	readonly items: readonly LiveUnattributedIndexedContract[];
}

export type ProtocolAttributionCoverage = 'seed-roots-only';

export interface LiveDashboardCapabilities {
	readonly walletContractRelationships: boolean;
	readonly protocolAttribution: ProtocolAttributionCoverage;
	readonly unattributedContractPlaceholders: boolean;
	readonly walletTransactionCounts: boolean;
	readonly walletLastActivity: boolean;
	readonly alerts: boolean;
	readonly historicalFindings: boolean;
	readonly alertDelivery: boolean;
	readonly leaderboard: boolean;
}

export interface LiveWalletAlertCoverage {
	readonly fromLedger: number;
	readonly toLedger: number;
}

export interface LiveWalletAlerts {
	readonly available: boolean;
	readonly reason: string;
	readonly deliveryAvailable: boolean;
	readonly deliveryReason: string;
	readonly coverage: LiveWalletAlertCoverage | null;
	readonly items: readonly HistoricalSecurityFinding[];
}

export interface LiveWalletResponse {
	readonly source: LiveDashboardSource;
	readonly query: LiveWalletQueryIdentity;
	readonly summary: LiveWalletSummary;
	readonly protocols: readonly LiveProtocolRelationship[];
	readonly contracts: LiveContractPage;
	readonly alerts: LiveWalletAlerts;
	readonly capabilities: LiveDashboardCapabilities;
}

export interface LiveLeaderboardRow {
	readonly rank: number;
	readonly accountAddress: StellarAccountAddress;
	readonly transactionCount: number;
	readonly successfulTransactionCount: number;
	readonly protocolCount: number;
	readonly contractCount: number;
	readonly lastInteractionAt: string;
}

export interface LiveLeaderboardResponse {
	readonly source: LiveDashboardSource;
	readonly metric: LeaderboardMetric;
	readonly rows: readonly LiveLeaderboardRow[];
}

export interface LiveProtocolCatalogResponse {
	readonly source: LiveDashboardSource;
	readonly protocols: readonly LiveProtocolCatalogEntry[];
	readonly alerts: readonly LiveContractUpgradeAlert[];
	readonly governance: LiveGovernanceMonitorResponse;
	readonly capabilities: LiveProtocolCatalogCapabilities;
}

export type LiveGovernanceMonitorResponse =
	| {
			readonly available: false;
			readonly reason: string;
			readonly observedAt: null;
			readonly stale: null;
			readonly controllerCount: 0;
			readonly controllers: readonly [];
			readonly alerts: readonly [];
	  }
	| {
			readonly available: true;
			readonly reason: string;
			readonly observedAt: string;
			readonly stale: boolean;
			readonly controllerCount: number;
			readonly controllers: readonly LiveGovernanceController[];
			readonly alerts: readonly LiveGovernanceAlert[];
	  };

export interface LiveGovernanceController {
	readonly accountId: StellarAccountAddress;
	readonly protocol: string;
	readonly role: string;
	readonly sources: readonly string[];
	readonly lastModifiedLedger: number;
	readonly policy: LiveGovernancePolicy;
}

export type LiveGovernanceClassification = 'strict' | 'weighted' | 'single-key' | 'locked';

export interface LiveGovernancePolicy {
	readonly thresholds: LiveGovernanceThresholds;
	readonly masterWeight: number;
	readonly signers: readonly LiveGovernanceSigner[];
	readonly classification: LiveGovernanceClassification;
}

export interface LiveGovernanceThresholds {
	readonly low: number;
	readonly medium: number;
	readonly high: number;
}

export interface LiveGovernanceSigner {
	readonly key: string;
	readonly type: 'ed25519_public_key' | 'sha256_hash' | 'preauth_tx';
	readonly weight: number;
}

export interface LiveGovernancePolicyAlert {
	readonly id: string;
	readonly type: 'classic-controller-policy-changed';
	readonly severity: 'critical';
	readonly detectedAt: string;
	readonly accountId: StellarAccountAddress;
	readonly protocol: string;
	readonly role: string;
	readonly beforeObservedAt: string;
	readonly afterLastModifiedLedger: number;
	readonly before: LiveGovernancePolicy;
	readonly after: LiveGovernancePolicy;
}

export interface LiveGovernanceActivityAlert {
	readonly id: string;
	readonly type: 'classic-controller-transaction-executed';
	readonly severity: 'high';
	readonly detectedAt: string;
	readonly occurredAt: string;
	readonly accountId: StellarAccountAddress;
	readonly protocol: string;
	readonly role: string;
	readonly transactionHash: StellarTransactionHash;
	readonly operations: readonly LiveGovernanceOperationPayload[];
}

export interface LiveGovernanceOperationPayload {
	readonly id: string;
	readonly type: string;
	readonly typeCode: number;
	readonly sourceAccount: StellarAccountAddress;
	readonly payload: Readonly<Record<string, LiveJsonValue>>;
}

export interface LiveJsonObject {
	readonly [key: string]: LiveJsonValue;
}

export interface LiveJsonArray extends ReadonlyArray<LiveJsonValue> {}

export type LiveJsonValue = null | boolean | number | string | LiveJsonArray | LiveJsonObject;

export type LiveGovernanceAlert = LiveGovernancePolicyAlert | LiveGovernanceActivityAlert;

export interface LiveProtocolCatalogCapabilities {
	readonly executableHistory: boolean;
	readonly upgradeAlerts: boolean;
	readonly multisigMonitoring: boolean;
	readonly multisigReason: string;
}

export interface LiveProtocolCatalogEntry {
	readonly protocolId: string;
	readonly contractCount: number;
	readonly versionObservationCount: number;
	readonly upgradeCount: number;
	readonly contracts: readonly LiveProtocolContractHistory[];
}

export interface LiveProtocolContractHistory {
	readonly contractAddress: StellarContractAddress;
	readonly roles: readonly LiveProtocolRootRole[];
	readonly usage: LiveProtocolContractUsage | null;
	readonly currentExecutable: LiveCurrentContractExecutable | null;
	readonly versions: readonly LiveContractExecutableVersion[];
}

export interface LiveProtocolRootRole {
	readonly role: string;
	readonly sourceUrl: string | null;
}

export interface LiveProtocolContractUsage {
	readonly firstLedger: number;
	readonly firstSeenAt: string | null;
	readonly lastLedger: number;
	readonly lastSeenAt: string | null;
	readonly transactionCount: number;
	readonly successfulTransactionCount: number;
}

export interface LiveCurrentContractExecutable {
	readonly ledger: number;
	readonly observedAt: string | null;
	readonly executableKind: string;
	readonly wasmHash: string | null;
	readonly exact: boolean;
}

export interface LiveContractExecutableVersion {
	readonly ledger: number;
	readonly observedAt: string | null;
	readonly applyOrder: number;
	readonly transactionHash: StellarTransactionHash;
	readonly executableKind: string;
	readonly wasmHash: string | null;
	readonly sourceKind: string;
	readonly activeBeforeWindow: boolean;
	readonly exactUpgrade: boolean;
}

export interface LiveContractUpgradeAlert {
	readonly alertId: string;
	readonly kind: 'contract_upgrade';
	readonly severity: 'info';
	readonly protocolId: string;
	readonly contractAddress: StellarContractAddress;
	readonly roles: readonly string[];
	readonly ledger: number;
	readonly observedAt: string | null;
	readonly transactionHash: StellarTransactionHash;
	readonly previousWasmHash: string | null;
	readonly newWasmHash: string | null;
}

export interface LiveExampleWallet {
	readonly accountAddress: StellarAccountAddress;
	readonly firstInteractionAt: string | null;
	readonly firstInteractionLedger: number;
}

export interface LiveExampleWalletsResponse {
	readonly source: LiveDashboardSource;
	/** The API sample is intentionally not presented as a ranking. */
	readonly selection: string;
	readonly wallets: readonly LiveExampleWallet[];
}

export interface LiveWalletRequestOptions extends LiveRequestOptions {
	readonly limit?: number;
	readonly cursor?: StellarContractAddress;
}

export interface LiveLeaderboardRequest extends LiveRequestOptions {
	readonly metric: LeaderboardMetric;
	readonly limit?: number;
	readonly offset?: number;
}

export interface LiveExampleWalletsRequest extends LiveRequestOptions {
	readonly limit?: number;
}

export interface LiveContractIndexRequest extends LiveRequestOptions {
	readonly limit?: number;
	readonly cursor?: StellarContractAddress;
}

export interface LiveRequestOptions {
	readonly signal?: AbortSignal;
	readonly timeoutMs?: number;
}

export interface LiveDashboardClientOptions {
	/** Defaults to the global fetch implementation. Useful for SSR and tests. */
	readonly fetch?: typeof fetch;
	/** Per-request timeout. Individual calls may override it. */
	readonly timeoutMs?: number;
}

export type LiveDashboardErrorKind =
	| 'configuration'
	| 'aborted'
	| 'timeout'
	| 'network'
	| 'http'
	| 'invalid-response';

export class LiveDashboardError extends Error {
	readonly kind: LiveDashboardErrorKind;

	constructor(kind: LiveDashboardErrorKind, message: string, options?: ErrorOptions) {
		super(message, options);
		this.name = 'LiveDashboardError';
		this.kind = kind;
	}
}

export class LiveDashboardHttpError extends LiveDashboardError {
	readonly status: number;
	readonly code: string;
	readonly details: unknown;

	constructor(status: number, code: string, message: string, details?: unknown) {
		super('http', message);
		this.name = 'LiveDashboardHttpError';
		this.status = status;
		this.code = code;
		this.details = details;
	}
}

export class LiveDashboardTimeoutError extends LiveDashboardError {
	readonly timeoutMs: number;

	constructor(timeoutMs: number) {
		super('timeout', `The Stargazer data service did not respond within ${timeoutMs} ms.`);
		this.name = 'LiveDashboardTimeoutError';
		this.timeoutMs = timeoutMs;
	}
}

export class LiveDashboardAbortedError extends LiveDashboardError {
	constructor(options?: ErrorOptions) {
		super('aborted', 'The Stargazer data request was cancelled.', options);
		this.name = 'LiveDashboardAbortedError';
	}
}

export class LiveDashboardNetworkError extends LiveDashboardError {
	constructor(options?: ErrorOptions) {
		super('network', 'The Stargazer data service could not be reached.', options);
		this.name = 'LiveDashboardNetworkError';
	}
}

export class LiveDashboardResponseError extends LiveDashboardError {
	readonly path: string;

	constructor(path: string, message: string, options?: ErrorOptions) {
		super('invalid-response', `Invalid response from ${path}: ${message}`, options);
		this.name = 'LiveDashboardResponseError';
		this.path = path;
	}
}

/** True only for the known startup state before wallet_activity is published. */
export function isLeaderboardUnavailable(error: unknown): error is LiveDashboardHttpError {
	return (
		error instanceof LiveDashboardHttpError &&
		error.status === 503 &&
		error.code === 'leaderboard_unavailable'
	);
}

/**
 * Narrow, read-only client for the API colocated with the NAS compact index.
 * It never imports or falls back to the synthetic demo repository.
 */
export class LiveDashboardClient {
	readonly baseUrl: string;
	readonly timeoutMs: number;
	readonly #fetch: typeof fetch;

	constructor(baseUrl: string, options: LiveDashboardClientOptions = {}) {
		this.baseUrl = normalizeBaseUrl(baseUrl);
		this.timeoutMs = validateTimeout(options.timeoutMs ?? DEFAULT_TIMEOUT_MS);
		this.#fetch = options.fetch ?? globalThis.fetch;
		if (typeof this.#fetch !== 'function') {
			throw new LiveDashboardError(
				'configuration',
				'No fetch implementation is available for the live dashboard client.'
			);
		}
	}

	async getWallet(
		address: StellarWalletAddress,
		options: LiveWalletRequestOptions = {}
	): Promise<LiveWalletResponse> {
		const limit = validateBoundedInteger(
			options.limit ?? DEFAULT_WALLET_CONTRACT_LIMIT,
			'wallet contract limit',
			1,
			MAX_WALLET_CONTRACT_LIMIT
		);
		const query = new URLSearchParams({ limit: String(limit) });
		if (options.cursor) query.set('cursor', options.cursor);
		const path = `/api/wallets/${encodeURIComponent(address)}?${query}`;
		const payload = await this.#request(path, options);
		return parseWalletResponse(payload, path);
	}

	async getLeaderboard({
		metric,
		limit = DEFAULT_LEADERBOARD_LIMIT,
		offset = 0,
		signal,
		timeoutMs
	}: LiveLeaderboardRequest): Promise<LiveLeaderboardResponse> {
		if (metric !== 'transactions' && metric !== 'protocols') {
			throw new LiveDashboardError(
				'configuration',
				'Leaderboard metric must be "transactions" or "protocols".'
			);
		}
		const safeLimit = validateBoundedInteger(
			limit,
			'leaderboard limit',
			1,
			MAX_LEADERBOARD_LIMIT
		);
		const safeOffset = validateBoundedInteger(offset, 'leaderboard offset', 0, Number.MAX_SAFE_INTEGER);
		const query = new URLSearchParams({
			metric,
			limit: String(safeLimit),
			offset: String(safeOffset)
		});
		const path = `/api/leaderboard?${query}`;
		const payload = await this.#request(path, { signal, timeoutMs });
		const response = parseLeaderboardResponse(payload, path);
		if (response.metric !== metric) {
			throw new LiveDashboardResponseError(
				path,
				`metric was ${JSON.stringify(response.metric)}, expected ${JSON.stringify(metric)}`
			);
		}
		return response;
	}

	/** Returns real, deterministic examples from the exact relationship index. */
	async getExampleWallets({
		limit = DEFAULT_EXAMPLE_WALLET_LIMIT,
		signal,
		timeoutMs
	}: LiveExampleWalletsRequest = {}): Promise<LiveExampleWalletsResponse> {
		const safeLimit = validateBoundedInteger(
			limit,
			'example wallet limit',
			1,
			MAX_EXAMPLE_WALLET_LIMIT
		);
		const path = `/api/examples/wallets?${new URLSearchParams({ limit: String(safeLimit) })}`;
		const payload = await this.#request(path, { signal, timeoutMs });
		return parseExampleWalletsResponse(payload, path);
	}

	/** Returns source-backed protocol roots and exact executable history. */
	async getProtocols(options: LiveRequestOptions = {}): Promise<LiveProtocolCatalogResponse> {
		const path = '/api/protocols';
		const payload = await this.#request(path, options);
		return parseProtocolCatalogResponse(payload, path);
	}

	/** Returns contract-ID ordered discoveries that do not yet have registry attribution. */
	async getUnattributedContracts(
		options: LiveContractIndexRequest = {}
	): Promise<LiveUnattributedContractIndexResponse> {
		const limit = validateBoundedInteger(
			options.limit ?? DEFAULT_CONTRACT_INDEX_LIMIT,
			'unattributed contract limit',
			1,
			MAX_CONTRACT_INDEX_LIMIT
		);
		const query = new URLSearchParams({ limit: String(limit) });
		if (options.cursor) query.set('cursor', options.cursor);
		const path = `/api/contracts/unattributed?${query}`;
		const payload = await this.#request(path, options);
		const response = parseContractIndexResponse(payload, path);
		if (response.attribution !== 'unattributed') {
			throw new LiveDashboardResponseError(
				path,
				`attribution was ${JSON.stringify(response.attribution)}, expected "unattributed"`
			);
		}
		const items = response.items.filter(
			(contract): contract is LiveUnattributedIndexedContract =>
				contract.attributionStatus === 'unattributed'
		);
		if (items.length !== response.items.length) {
			throw new LiveDashboardResponseError(
				path,
				'unattributed endpoint returned a verified registry contract'
			);
		}
		return { ...response, attribution: 'unattributed', items };
	}

	async #request(path: string, options: LiveRequestOptions): Promise<unknown> {
		const timeoutMs = validateTimeout(options.timeoutMs ?? this.timeoutMs);
		if (options.signal?.aborted) {
			throw new LiveDashboardAbortedError({ cause: options.signal.reason });
		}

		const controller = new AbortController();
		let timedOut = false;
		const abortFromCaller = () => controller.abort(options.signal?.reason);
		options.signal?.addEventListener('abort', abortFromCaller, { once: true });
		const timeout = setTimeout(() => {
			timedOut = true;
			controller.abort(new Error(`Request exceeded ${timeoutMs} ms`));
		}, timeoutMs);

		try {
			let response: Response;
			try {
				response = await this.#fetch(`${this.baseUrl}${path}`, {
					method: 'GET',
					headers: { accept: 'application/json' },
					cache: 'no-store',
					signal: controller.signal
				});
			} catch (error) {
				if (timedOut) throw new LiveDashboardTimeoutError(timeoutMs);
				if (options.signal?.aborted) {
					throw new LiveDashboardAbortedError({ cause: options.signal.reason ?? error });
				}
				throw new LiveDashboardNetworkError({ cause: error });
			}

			const payload = await readJson(response, path);
			if (timedOut) throw new LiveDashboardTimeoutError(timeoutMs);
			if (options.signal?.aborted) {
				throw new LiveDashboardAbortedError({ cause: options.signal.reason });
			}
			if (!response.ok) throw parseHttpError(response.status, payload);
			return payload;
		} catch (error) {
			// Fetch implementations can surface body-stream aborts differently from
			// request aborts, so classify them using the controller state here too.
			if (timedOut && !(error instanceof LiveDashboardHttpError)) {
				throw new LiveDashboardTimeoutError(timeoutMs);
			}
			if (options.signal?.aborted && !(error instanceof LiveDashboardHttpError)) {
				throw new LiveDashboardAbortedError({ cause: options.signal.reason ?? error });
			}
			throw error;
		} finally {
			clearTimeout(timeout);
			options.signal?.removeEventListener('abort', abortFromCaller);
		}
	}
}

function normalizeBaseUrl(baseUrl: string): string {
	const value = baseUrl.trim();
	let parsed: URL;
	try {
		parsed = new URL(value);
	} catch (error) {
		throw new LiveDashboardError(
			'configuration',
			'Live dashboard base URL must be an absolute HTTP(S) URL.',
			{ cause: error }
		);
	}
	if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
		throw new LiveDashboardError(
			'configuration',
			'Live dashboard base URL must use HTTP or HTTPS.'
		);
	}
	if (parsed.username || parsed.password || parsed.search || parsed.hash) {
		throw new LiveDashboardError(
			'configuration',
			'Live dashboard base URL cannot contain credentials, a query, or a fragment.'
		);
	}
	return parsed.toString().replace(/\/$/, '');
}

function validateTimeout(value: number): number {
	return validateBoundedInteger(value, 'request timeout', 1, 300_000);
}

function validateBoundedInteger(value: number, label: string, min: number, max: number): number {
	if (!Number.isSafeInteger(value) || value < min || value > max) {
		throw new LiveDashboardError(
			'configuration',
			`${label} must be a safe integer between ${min} and ${max}.`
		);
	}
	return value;
}

async function readJson(response: Response, path: string): Promise<unknown> {
	let text: string;
	try {
		text = await response.text();
	} catch (error) {
		throw new LiveDashboardResponseError(path, 'the response body could not be read', {
			cause: error
		});
	}
	if (text.length === 0) {
		throw new LiveDashboardResponseError(path, 'the response body was empty');
	}
	try {
		return JSON.parse(text) as unknown;
	} catch (error) {
		throw new LiveDashboardResponseError(path, 'the response body was not valid JSON', {
			cause: error
		});
	}
}

function parseHttpError(status: number, payload: unknown): LiveDashboardHttpError {
	if (isObject(payload) && isObject(payload.error)) {
		const code = optionalString(payload.error.code) ?? `http_${status}`;
		const message = optionalString(payload.error.message) ?? `Stargazer data service returned HTTP ${status}.`;
		return new LiveDashboardHttpError(status, code, message, payload);
	}
	return new LiveDashboardHttpError(
		status,
		`http_${status}`,
		`Stargazer data service returned HTTP ${status}.`,
		payload
	);
}

function parseWalletResponse(payload: unknown, path: string): LiveWalletResponse {
	try {
		const value = objectValue(payload, 'response');
		const source = parseSource(value.source);
		const query = parseWalletIdentity(value.query);
		const summary = parseWalletSummary(value.summary);
		if (summary.accountAddress !== query.accountAddress) {
			throw new Error('summary.accountAddress does not match query.accountAddress');
		}
		const protocols = arrayValue(value.protocols, 'protocols').map(parseProtocolRelationship);
		const contracts = parseContractPage(value.contracts);
		const alerts = parseAlerts(value.alerts);
		const capabilities = parseCapabilities(value.capabilities);
		if (summary.protocolCount !== protocols.length) {
			throw new Error('summary.protocolCount does not match source-backed protocol count');
		}
		if (
			!capabilities.unattributedContractPlaceholders &&
			contracts.items.some((contract) => contract.attributionStatus === 'unattributed')
		) {
			throw new Error(
				'unattributed contract rows require capabilities.unattributedContractPlaceholders'
			);
		}
		const pageUnattributedCount = contracts.items.filter(
			(contract) => contract.attributionStatus === 'unattributed'
		).length;
		if (pageUnattributedCount > summary.unattributedContractCount) {
			throw new Error('contracts page contains more unattributed rows than the wallet summary');
		}
		if (alerts.available && summary.alertCount !== alerts.items.length) {
			throw new Error('summary.alertCount does not match historical finding count');
		}
		if (!alerts.available && summary.alertCount !== null) {
			throw new Error('summary.alertCount must be null when historical findings are unavailable');
		}
		if (capabilities.alerts !== alerts.available) {
			throw new Error('capabilities.alerts does not match historical finding availability');
		}
		if (capabilities.historicalFindings !== alerts.available) {
			throw new Error(
				'capabilities.historicalFindings does not match historical finding availability'
			);
		}
		if (capabilities.alertDelivery !== alerts.deliveryAvailable) {
			throw new Error('capabilities.alertDelivery does not match alert delivery availability');
		}
		return {
			source,
			query,
			summary,
			protocols,
			contracts,
			alerts,
			capabilities
		};
	} catch (error) {
		if (error instanceof LiveDashboardResponseError) throw error;
		throw new LiveDashboardResponseError(path, errorMessage(error), { cause: error });
	}
}

function parseLeaderboardResponse(payload: unknown, path: string): LiveLeaderboardResponse {
	try {
		const value = objectValue(payload, 'response');
		return {
			source: parseSource(value.source),
			metric: leaderboardMetric(value.metric, 'metric'),
			rows: arrayValue(value.rows, 'rows').map((row, index) => parseLeaderboardRow(row, index))
		};
	} catch (error) {
		if (error instanceof LiveDashboardResponseError) throw error;
		throw new LiveDashboardResponseError(path, errorMessage(error), { cause: error });
	}
}

function parseExampleWalletsResponse(payload: unknown, path: string): LiveExampleWalletsResponse {
	try {
		const value = objectValue(payload, 'response');
		return {
			source: parseSource(value.source),
			selection: stringValue(value.selection, 'selection'),
			wallets: arrayValue(value.wallets, 'wallets').map((wallet, index) => {
				const field = `wallets[${index}]`;
				const item = objectValue(wallet, field);
				return {
					accountAddress: accountAddress(item.accountAddress, `${field}.accountAddress`),
					firstInteractionAt: nullableTimestamp(
						item.firstInteractionAt,
						`${field}.firstInteractionAt`
					),
					firstInteractionLedger: nonNegativeInteger(
						item.firstInteractionLedger,
						`${field}.firstInteractionLedger`
					)
				};
			})
		};
	} catch (error) {
		if (error instanceof LiveDashboardResponseError) throw error;
		throw new LiveDashboardResponseError(path, errorMessage(error), { cause: error });
	}
}

function parseProtocolCatalogResponse(payload: unknown, path: string): LiveProtocolCatalogResponse {
	try {
		const value = objectValue(payload, 'response');
		const protocols = arrayValue(value.protocols, 'protocols').map(parseProtocolCatalogEntry);
		const alerts = arrayValue(value.alerts, 'alerts').map(parseContractUpgradeAlert);
		return {
			source: parseSource(value.source),
			protocols,
			alerts,
			governance: parseGovernanceMonitor(value.governance),
			capabilities: parseProtocolCatalogCapabilities(value.capabilities)
		};
	} catch (error) {
		if (error instanceof LiveDashboardResponseError) throw error;
		throw new LiveDashboardResponseError(path, errorMessage(error), { cause: error });
	}
}

function parseContractIndexResponse(payload: unknown, path: string): LiveContractIndexResponse {
	try {
		const value = objectValue(payload, 'response');
		const attribution = contractIndexAttribution(value.attribution, 'attribution');
		const items = arrayValue(value.items, 'items').map(parseIndexedContract);
		const limit = nonNegativeInteger(value.limit, 'limit');
		const nextCursor =
			value.nextCursor === null ? null : contractAddress(value.nextCursor, 'nextCursor');
		if (items.length > limit) throw new Error('items.length exceeds limit');
		for (let index = 1; index < items.length; index += 1) {
			const previous = items[index - 1];
			const current = items[index];
			if (
				previous &&
				current &&
				compareStellarStrKeyOrder(previous.contractAddress, current.contractAddress) >= 0
			) {
				throw new Error('items must be uniquely sorted by contractAddress');
			}
		}
		if (nextCursor !== null && items.at(-1)?.contractAddress !== nextCursor) {
			throw new Error('nextCursor must equal the final returned contractAddress');
		}
		if (
			attribution === 'unattributed' &&
			items.some((contract) => contract.attributionStatus !== 'unattributed')
		) {
			throw new Error('unattributed response cannot contain verified registry contracts');
		}
		return {
			source: parseSource(value.source),
			attribution,
			items,
			limit,
			nextCursor
		};
	} catch (error) {
		if (error instanceof LiveDashboardResponseError) throw error;
		throw new LiveDashboardResponseError(path, errorMessage(error), { cause: error });
	}
}

function parseIndexedContract(input: unknown, index: number): LiveIndexedContract {
	const field = `items[${index}]`;
	const value = objectValue(input, field);
	const parsedContractAddress = contractAddress(
		value.contractAddress,
		`${field}.contractAddress`
	);
	const attributionStatus = contractAttributionStatus(
		value.attributionStatus,
		`${field}.attributionStatus`
	);
	const placeholderProtocolId =
		value.placeholderProtocolId === null
			? null
			: contractAddress(value.placeholderProtocolId, `${field}.placeholderProtocolId`);
	const protocols = arrayValue(value.protocols, `${field}.protocols`).map(
		(protocol, protocolIndex) => {
			const protocolField = `${field}.protocols[${protocolIndex}]`;
			const protocolValue = objectValue(protocol, protocolField);
			return {
				protocolId: stringValue(protocolValue.protocolId, `${protocolField}.protocolId`),
				role: stringValue(protocolValue.role, `${protocolField}.role`),
				sourceUrl: nullableUrl(protocolValue.sourceUrl, `${protocolField}.sourceUrl`)
			};
		}
	);
	const firstLedger = nonNegativeInteger(value.firstLedger, `${field}.firstLedger`);
	const lastLedger = nonNegativeInteger(value.lastLedger, `${field}.lastLedger`);
	if (firstLedger > lastLedger) throw new Error(`${field} ledger range is inverted`);
	const transactionCount = nonNegativeInteger(
		value.transactionCount,
		`${field}.transactionCount`
	);
	const successfulTransactionCount = nonNegativeInteger(
		value.successfulTransactionCount,
		`${field}.successfulTransactionCount`
	);
	if (successfulTransactionCount > transactionCount) {
		throw new Error(`${field}.successfulTransactionCount exceeds transactionCount`);
	}
	const contract = {
		contractAddress: parsedContractAddress,
		firstLedger,
		firstSeenAt: nullableTimestamp(value.firstSeenAt, `${field}.firstSeenAt`),
		lastLedger,
		lastSeenAt: nullableTimestamp(value.lastSeenAt, `${field}.lastSeenAt`),
		transactionCount,
		successfulTransactionCount,
		currentExecutable:
			value.currentExecutable === null
				? null
				: parseCurrentContractExecutable(
						value.currentExecutable,
						`${field}.currentExecutable`
					)
	};
	if (attributionStatus === 'verified-registry') {
		if (placeholderProtocolId !== null || protocols.length === 0) {
			throw new Error(`${field} verified registry attribution is inconsistent`);
		}
		return {
			...contract,
			attributionStatus,
			placeholderProtocolId: null,
			protocols
		};
	}
	if (placeholderProtocolId !== parsedContractAddress || protocols.length !== 0) {
		throw new Error(`${field} unattributed placeholder is inconsistent`);
	}
	return {
		...contract,
		attributionStatus,
		placeholderProtocolId: parsedContractAddress,
		protocols: []
	};
}

function parseGovernanceMonitor(input: unknown): LiveGovernanceMonitorResponse {
	const value = objectValue(input, 'governance');
	const reason = stringValue(value.reason, 'governance.reason');
	if (value.available === false) {
		const controllers = arrayValue(value.controllers, 'governance.controllers');
		const alerts = arrayValue(value.alerts, 'governance.alerts');
		if (value.observedAt !== null) {
			throw new Error('governance.observedAt must be null when monitoring is unavailable');
		}
		if (value.stale !== null) {
			throw new Error('governance.stale must be null when monitoring is unavailable');
		}
		if (value.controllerCount !== 0 || controllers.length !== 0 || alerts.length !== 0) {
			throw new Error('unavailable governance monitoring must not contain observations');
		}
		return {
			available: false,
			reason,
			observedAt: null,
			stale: null,
			controllerCount: 0,
			controllers: [],
			alerts: []
		};
	}
	if (value.available !== true) throw new Error('governance.available must be a boolean');

	const observedAt = timestampValue(value.observedAt, 'governance.observedAt');
	const stale = booleanValue(value.stale, 'governance.stale');
	const controllerCount = nonNegativeInteger(
		value.controllerCount,
		'governance.controllerCount'
	);
	const controllers = arrayValue(value.controllers, 'governance.controllers').map(
		(controller, index) => parseGovernanceController(controller, index)
	);
	const alerts = arrayValue(value.alerts, 'governance.alerts').map((alert, index) =>
		parseGovernanceAlert(alert, index)
	);
	if (controllerCount === 0 || controllerCount !== controllers.length) {
		throw new Error('governance.controllerCount must match a non-empty controllers array');
	}
	if (new Set(controllers.map((controller) => controller.accountId)).size !== controllers.length) {
		throw new Error('governance.controllers contains duplicate accounts');
	}
	if (new Set(alerts.map((alert) => alert.id)).size !== alerts.length) {
		throw new Error('governance.alerts contains duplicate alert IDs');
	}
	return {
		available: true,
		reason,
		observedAt,
		stale,
		controllerCount,
		controllers,
		alerts
	};
}

function parseGovernanceAlert(input: unknown, index: number): LiveGovernanceAlert {
	const value = objectValue(input, `governance.alerts[${index}]`);
	if (value.type === 'classic-controller-policy-changed') {
		return parseGovernancePolicyAlert(value, index);
	}
	if (value.type === 'classic-controller-transaction-executed') {
		return parseGovernanceActivityAlert(value, index);
	}
	throw new Error(`governance.alerts[${index}].type is invalid`);
}

function parseGovernanceController(input: unknown, index: number): LiveGovernanceController {
	const field = `governance.controllers[${index}]`;
	const value = objectValue(input, field);
	const accountId = accountAddress(value.accountId, `${field}.accountId`);
	const sources = arrayValue(value.sources, `${field}.sources`).map((source, sourceIndex) =>
		httpsUrl(source, `${field}.sources[${sourceIndex}]`)
	);
	if (sources.length === 0) throw new Error(`${field}.sources must not be empty`);
	return {
		accountId,
		protocol: stringValue(value.protocol, `${field}.protocol`),
		role: stringValue(value.role, `${field}.role`),
		sources,
		lastModifiedLedger: nonNegativeInteger(
			value.lastModifiedLedger,
			`${field}.lastModifiedLedger`
		),
		policy: parseGovernancePolicy(value.policy, `${field}.policy`, accountId)
	};
}

function parseGovernancePolicyAlert(input: unknown, index: number): LiveGovernancePolicyAlert {
	const field = `governance.alerts[${index}]`;
	const value = objectValue(input, field);
	const id = stringValue(value.id, `${field}.id`);
	if (!/^[0-9a-f]{64}$/.test(id)) throw new Error(`${field}.id must be a lowercase SHA-256 digest`);
	if (value.type !== 'classic-controller-policy-changed') {
		throw new Error(`${field}.type is invalid`);
	}
	if (value.severity !== 'critical') throw new Error(`${field}.severity is invalid`);
	const accountId = accountAddress(value.accountId, `${field}.accountId`);
	const before = parseGovernancePolicy(value.before, `${field}.before`, accountId);
	const after = parseGovernancePolicy(value.after, `${field}.after`, accountId);
	if (JSON.stringify(before) === JSON.stringify(after)) {
		throw new Error(`${field} does not contain a policy change`);
	}
	return {
		id,
		type: 'classic-controller-policy-changed',
		severity: 'critical',
		detectedAt: timestampValue(value.detectedAt, `${field}.detectedAt`),
		accountId,
		protocol: stringValue(value.protocol, `${field}.protocol`),
		role: stringValue(value.role, `${field}.role`),
		beforeObservedAt: timestampValue(value.beforeObservedAt, `${field}.beforeObservedAt`),
		afterLastModifiedLedger: nonNegativeInteger(
			value.afterLastModifiedLedger,
			`${field}.afterLastModifiedLedger`
		),
		before,
		after
	};
}

function parseGovernanceActivityAlert(
	input: unknown,
	index: number
): LiveGovernanceActivityAlert {
	const field = `governance.alerts[${index}]`;
	const value = objectValue(input, field);
	const id = stringValue(value.id, `${field}.id`);
	if (!/^[0-9a-f]{64}$/.test(id)) throw new Error(`${field}.id must be a lowercase SHA-256 digest`);
	if (value.type !== 'classic-controller-transaction-executed') {
		throw new Error(`${field}.type is invalid`);
	}
	if (value.severity !== 'high') throw new Error(`${field}.severity is invalid`);
	const accountId = accountAddress(value.accountId, `${field}.accountId`);
	const operations = arrayValue(value.operations, `${field}.operations`).map(
		(operation, operationIndex) =>
			parseGovernanceOperationPayload(operation, operationIndex, field, accountId)
	);
	if (operations.length === 0 || operations.length > 100) {
		throw new Error(`${field}.operations must contain between 1 and 100 operations`);
	}
	if (new Set(operations.map((operation) => operation.id)).size !== operations.length) {
		throw new Error(`${field}.operations contains duplicate operation IDs`);
	}
	return {
		id,
		type: 'classic-controller-transaction-executed',
		severity: 'high',
		detectedAt: timestampValue(value.detectedAt, `${field}.detectedAt`),
		occurredAt: timestampValue(value.occurredAt, `${field}.occurredAt`),
		accountId,
		protocol: stringValue(value.protocol, `${field}.protocol`),
		role: stringValue(value.role, `${field}.role`),
		transactionHash: transactionHash(value.transactionHash, `${field}.transactionHash`),
		operations
	};
}

function parseGovernanceOperationPayload(
	input: unknown,
	index: number,
	parentField: string,
	accountId: StellarAccountAddress
): LiveGovernanceOperationPayload {
	const field = `${parentField}.operations[${index}]`;
	const value = objectValue(input, field);
	const sourceAccount = accountAddress(value.sourceAccount, `${field}.sourceAccount`);
	if (sourceAccount !== accountId) {
		throw new Error(`${field}.sourceAccount does not match the tracked controller`);
	}
	return {
		id: decimalString(value.id, `${field}.id`),
		type: stringValue(value.type, `${field}.type`),
		typeCode: nonNegativeInteger(value.typeCode, `${field}.typeCode`),
		sourceAccount,
		payload: jsonObjectValue(value.payload, `${field}.payload`)
	};
}

function parseGovernancePolicy(
	input: unknown,
	field: string,
	accountId: StellarAccountAddress
): LiveGovernancePolicy {
	const value = objectValue(input, field);
	const thresholdValue = objectValue(value.thresholds, `${field}.thresholds`);
	const thresholds = {
		low: byteValue(thresholdValue.low, `${field}.thresholds.low`),
		medium: byteValue(thresholdValue.medium, `${field}.thresholds.medium`),
		high: byteValue(thresholdValue.high, `${field}.thresholds.high`)
	};
	const masterWeight = byteValue(value.masterWeight, `${field}.masterWeight`);
	const signers = arrayValue(value.signers, `${field}.signers`).map((signer, signerIndex) =>
		parseGovernanceSigner(signer, `${field}.signers[${signerIndex}]`)
	);
	if (signers.length === 0) throw new Error(`${field}.signers must not be empty`);
	const masterSigner = signers.find(
		(signer) => signer.type === 'ed25519_public_key' && signer.key === accountId
	);
	if (masterSigner?.weight !== masterWeight) {
		throw new Error(`${field}.masterWeight does not match the account's master signer`);
	}
	for (let index = 1; index < signers.length; index += 1) {
		const previous = signers[index - 1];
		const current = signers[index];
		if (!previous || !current) continue;
		const previousIdentity = `${previous.type}:${previous.key}`;
		const currentIdentity = `${current.type}:${current.key}`;
		if (previousIdentity >= currentIdentity) {
			throw new Error(`${field}.signers must be uniquely sorted by type and key`);
		}
	}
	const classification = governanceClassification(value.classification, `${field}.classification`);
	const expectedClassification = classifyGovernancePolicy(thresholds, signers);
	if (classification !== expectedClassification) {
		throw new Error(`${field}.classification does not match its signer and threshold policy`);
	}
	return { thresholds, masterWeight, signers, classification };
}

function parseGovernanceSigner(input: unknown, field: string): LiveGovernanceSigner {
	const value = objectValue(input, field);
	const type = value.type;
	if (type !== 'ed25519_public_key' && type !== 'sha256_hash' && type !== 'preauth_tx') {
		throw new Error(`${field}.type is invalid`);
	}
	const key = stringValue(value.key, `${field}.key`);
	if (type === 'ed25519_public_key') {
		accountAddress(key, `${field}.key`);
	} else if (!/^[0-9a-f]{64}$/.test(key)) {
		throw new Error(`${field}.key must be a lowercase 32-byte digest`);
	}
	return { key, type, weight: byteValue(value.weight, `${field}.weight`) };
}

function governanceClassification(
	input: unknown,
	field: string
): LiveGovernanceClassification {
	if (input !== 'strict' && input !== 'weighted' && input !== 'single-key' && input !== 'locked') {
		throw new Error(`${field} is invalid`);
	}
	return input;
}

function classifyGovernancePolicy(
	thresholds: LiveGovernanceThresholds,
	signers: readonly LiveGovernanceSigner[]
): LiveGovernanceClassification {
	const positiveSigners = signers.filter((signer) => signer.weight > 0);
	if (positiveSigners.length === 0) return 'locked';
	if (positiveSigners.length === 1) return 'single-key';
	const weights = new Set(positiveSigners.map((signer) => signer.weight));
	const thresholdValues = [thresholds.low, thresholds.medium, thresholds.high];
	if (weights.size === 1 && thresholdValues.every((value) => value === thresholdValues[0])) {
		const signerWeight = positiveSigners[0]?.weight ?? 0;
		const effectiveThreshold = Math.max(thresholds.low, 1);
		const requiredSigners = Math.ceil(effectiveThreshold / signerWeight);
		if (requiredSigners > 1 && requiredSigners <= positiveSigners.length) return 'strict';
	}
	return 'weighted';
}

function parseProtocolCatalogEntry(input: unknown, index: number): LiveProtocolCatalogEntry {
	const field = `protocols[${index}]`;
	const value = objectValue(input, field);
	const contracts = arrayValue(value.contracts, `${field}.contracts`).map((contract, contractIndex) =>
		parseProtocolContractHistory(contract, `${field}.contracts[${contractIndex}]`)
	);
	const contractCount = nonNegativeInteger(value.contractCount, `${field}.contractCount`);
	const versionObservationCount = nonNegativeInteger(
		value.versionObservationCount,
		`${field}.versionObservationCount`
	);
	const upgradeCount = nonNegativeInteger(value.upgradeCount, `${field}.upgradeCount`);
	if (contractCount !== contracts.length) {
		throw new Error(`${field}.contractCount does not match contracts.length`);
	}
	const parsedVersionCount = contracts.reduce((count, contract) => count + contract.versions.length, 0);
	const parsedUpgradeCount = contracts.reduce(
		(count, contract) => count + contract.versions.filter((version) => version.exactUpgrade).length,
		0
	);
	if (versionObservationCount !== parsedVersionCount) {
		throw new Error(`${field}.versionObservationCount does not match contract histories`);
	}
	if (upgradeCount !== parsedUpgradeCount) {
		throw new Error(`${field}.upgradeCount does not match exact upgrade observations`);
	}
	return {
		protocolId: stringValue(value.protocolId, `${field}.protocolId`),
		contractCount,
		versionObservationCount,
		upgradeCount,
		contracts
	};
}

function parseProtocolContractHistory(
	input: unknown,
	field: string
): LiveProtocolContractHistory {
	const value = objectValue(input, field);
	return {
		contractAddress: contractAddress(value.contractAddress, `${field}.contractAddress`),
		roles: arrayValue(value.roles, `${field}.roles`).map((role, roleIndex) => {
			const roleField = `${field}.roles[${roleIndex}]`;
			const roleValue = objectValue(role, roleField);
			return {
				role: stringValue(roleValue.role, `${roleField}.role`),
				sourceUrl: nullableUrl(roleValue.sourceUrl, `${roleField}.sourceUrl`)
			};
		}),
		usage:
			value.usage === null ? null : parseProtocolContractUsage(value.usage, `${field}.usage`),
		currentExecutable:
			value.currentExecutable === null
				? null
				: parseCurrentContractExecutable(value.currentExecutable, `${field}.currentExecutable`),
		versions: arrayValue(value.versions, `${field}.versions`).map((version, versionIndex) =>
			parseContractExecutableVersion(version, `${field}.versions[${versionIndex}]`)
		)
	};
}

function parseProtocolContractUsage(input: unknown, field: string): LiveProtocolContractUsage {
	const value = objectValue(input, field);
	const firstLedger = nonNegativeInteger(value.firstLedger, `${field}.firstLedger`);
	const lastLedger = nonNegativeInteger(value.lastLedger, `${field}.lastLedger`);
	const transactionCount = nonNegativeInteger(value.transactionCount, `${field}.transactionCount`);
	const successfulTransactionCount = nonNegativeInteger(
		value.successfulTransactionCount,
		`${field}.successfulTransactionCount`
	);
	if (firstLedger > lastLedger) throw new Error(`${field} ledger range is inverted`);
	if (successfulTransactionCount > transactionCount) {
		throw new Error(`${field}.successfulTransactionCount exceeds transactionCount`);
	}
	return {
		firstLedger,
		firstSeenAt: nullableTimestamp(value.firstSeenAt, `${field}.firstSeenAt`),
		lastLedger,
		lastSeenAt: nullableTimestamp(value.lastSeenAt, `${field}.lastSeenAt`),
		transactionCount,
		successfulTransactionCount
	};
}

function parseCurrentContractExecutable(
	input: unknown,
	field: string
): LiveCurrentContractExecutable {
	const value = objectValue(input, field);
	return {
		ledger: nonNegativeInteger(value.ledger, `${field}.ledger`),
		observedAt: nullableTimestamp(value.observedAt, `${field}.observedAt`),
		executableKind: stringValue(value.executableKind, `${field}.executableKind`),
		wasmHash: nullableHash(value.wasmHash, `${field}.wasmHash`),
		exact: booleanValue(value.exact, `${field}.exact`)
	};
}

function parseContractExecutableVersion(
	input: unknown,
	field: string
): LiveContractExecutableVersion {
	const value = objectValue(input, field);
	const sourceKind = stringValue(value.sourceKind, `${field}.sourceKind`);
	const activeBeforeWindow = booleanValue(
		value.activeBeforeWindow,
		`${field}.activeBeforeWindow`
	);
	const exactUpgrade = booleanValue(value.exactUpgrade, `${field}.exactUpgrade`);
	if (exactUpgrade && (sourceKind !== 'updated' || activeBeforeWindow)) {
		throw new Error(`${field}.exactUpgrade is inconsistent with its source provenance`);
	}
	return {
		ledger: nonNegativeInteger(value.ledger, `${field}.ledger`),
		observedAt: nullableTimestamp(value.observedAt, `${field}.observedAt`),
		applyOrder: nonNegativeInteger(value.applyOrder, `${field}.applyOrder`),
		transactionHash: transactionHash(value.transactionHash, `${field}.transactionHash`),
		executableKind: stringValue(value.executableKind, `${field}.executableKind`),
		wasmHash: nullableHash(value.wasmHash, `${field}.wasmHash`),
		sourceKind,
		activeBeforeWindow,
		exactUpgrade
	};
}

function parseContractUpgradeAlert(input: unknown, index: number): LiveContractUpgradeAlert {
	const field = `alerts[${index}]`;
	const value = objectValue(input, field);
	if (value.kind !== 'contract_upgrade') throw new Error(`${field}.kind is invalid`);
	if (value.severity !== 'info') throw new Error(`${field}.severity is invalid`);
	return {
		alertId: stringValue(value.alertId, `${field}.alertId`),
		kind: 'contract_upgrade',
		severity: 'info',
		protocolId: stringValue(value.protocolId, `${field}.protocolId`),
		contractAddress: contractAddress(value.contractAddress, `${field}.contractAddress`),
		roles: arrayValue(value.roles, `${field}.roles`).map((role, roleIndex) =>
			stringValue(role, `${field}.roles[${roleIndex}]`)
		),
		ledger: nonNegativeInteger(value.ledger, `${field}.ledger`),
		observedAt: nullableTimestamp(value.observedAt, `${field}.observedAt`),
		transactionHash: transactionHash(value.transactionHash, `${field}.transactionHash`),
		previousWasmHash: nullableHash(value.previousWasmHash, `${field}.previousWasmHash`),
		newWasmHash: nullableHash(value.newWasmHash, `${field}.newWasmHash`)
	};
}

function parseProtocolCatalogCapabilities(input: unknown): LiveProtocolCatalogCapabilities {
	const value = objectValue(input, 'capabilities');
	return {
		executableHistory: booleanValue(value.executableHistory, 'capabilities.executableHistory'),
		upgradeAlerts: booleanValue(value.upgradeAlerts, 'capabilities.upgradeAlerts'),
		multisigMonitoring: booleanValue(
			value.multisigMonitoring,
			'capabilities.multisigMonitoring'
		),
		multisigReason: stringValue(value.multisigReason, 'capabilities.multisigReason')
	};
}

function parseSource(input: unknown): LiveDashboardSource {
	const value = objectValue(input, 'source');
	if (value.mode !== 'live') throw new Error('source.mode must be "live"');
	if (value.network !== 'Stellar Mainnet') {
		throw new Error('source.network must be "Stellar Mainnet"');
	}
	const indexedLedgerStart = optionalNonNegativeInteger(
		value.indexedLedgerStart,
		'source.indexedLedgerStart'
	);
	const indexedLedgerEnd = optionalNonNegativeInteger(
		value.indexedLedgerEnd,
		'source.indexedLedgerEnd'
	);
	const indexedContractTransactionCount = nonNegativeInteger(
		value.indexedContractTransactionCount,
		'source.indexedContractTransactionCount'
	);
	if (
		indexedLedgerStart !== undefined &&
		indexedLedgerEnd !== undefined &&
		indexedLedgerStart > indexedLedgerEnd
	) {
		throw new Error('source indexed ledger range is inverted');
	}
	return {
		mode: 'live',
		label: stringValue(value.label, 'source.label'),
		indexedFrom: timestampValue(value.indexedFrom, 'source.indexedFrom'),
		asOf: timestampValue(value.asOf, 'source.asOf'),
		network: 'Stellar Mainnet',
		indexedLedgerStart,
		indexedLedgerEnd,
		indexedContractTransactionCount,
		disclaimer: optionalString(value.disclaimer)
	};
}

function parseWalletIdentity(input: unknown): LiveWalletQueryIdentity {
	const value = objectValue(input, 'query');
	return {
		inputAddress: stringValue(value.inputAddress, 'query.inputAddress'),
		accountAddress: accountAddress(value.accountAddress, 'query.accountAddress'),
		muxedId: nullableDecimalString(value.muxedId, 'query.muxedId')
	};
}

function parseWalletSummary(input: unknown): LiveWalletSummary {
	const value = objectValue(input, 'summary');
	const contractCount = nonNegativeInteger(value.contractCount, 'summary.contractCount');
	const transactionCount = nullableNonNegativeInteger(
		value.transactionCount,
		'summary.transactionCount'
	);
	const transactionRank = nullableNonNegativeInteger(
		value.transactionRank,
		'summary.transactionRank'
	);
	const protocolRank = nullableNonNegativeInteger(value.protocolRank, 'summary.protocolRank');
	const rankedWalletCount = nullableNonNegativeInteger(
		value.rankedWalletCount,
		'summary.rankedWalletCount'
	);
	const unattributedContractCount = nonNegativeInteger(
		value.unattributedContractCount,
		'summary.unattributedContractCount'
	);
	if (unattributedContractCount > contractCount) {
		throw new Error('summary.unattributedContractCount exceeds summary.contractCount');
	}
	if (
		(transactionRank === null) !== (protocolRank === null) ||
		(transactionRank === null) !== (rankedWalletCount === null)
	) {
		throw new Error('summary activity ranks must be available or unavailable together');
	}
	if (transactionCount === null && transactionRank !== null) {
		throw new Error('summary.transactionRank requires summary.transactionCount');
	}
	if (
		rankedWalletCount !== null &&
		transactionRank !== null &&
		protocolRank !== null &&
		(transactionRank === 0 ||
			protocolRank === 0 ||
			transactionRank > rankedWalletCount ||
			protocolRank > rankedWalletCount)
	) {
		throw new Error('summary activity ranks must be within rankedWalletCount');
	}
	return {
		accountAddress: accountAddress(value.accountAddress, 'summary.accountAddress'),
		firstInteractionAt: nullableTimestamp(value.firstInteractionAt, 'summary.firstInteractionAt'),
		lastInteractionAt: nullableTimestamp(value.lastInteractionAt, 'summary.lastInteractionAt'),
		transactionCount,
		successfulTransactionCount: nullableNonNegativeInteger(
			value.successfulTransactionCount,
			'summary.successfulTransactionCount'
		),
		transactionRank,
		protocolRank,
		rankedWalletCount,
		protocolCount: nonNegativeInteger(value.protocolCount, 'summary.protocolCount'),
		contractCount,
		unattributedContractCount,
		alertCount: nullableNonNegativeInteger(value.alertCount, 'summary.alertCount')
	};
}

function parseProtocolRelationship(input: unknown, index: number): LiveProtocolRelationship {
	const field = `protocols[${index}]`;
	const value = objectValue(input, field);
	return {
		protocolId: stringValue(value.protocolId, `${field}.protocolId`),
		contractCount: nonNegativeInteger(value.contractCount, `${field}.contractCount`),
		firstInteractionAt: nullableTimestamp(value.firstInteractionAt, `${field}.firstInteractionAt`),
		firstInteractionLedger: nonNegativeInteger(
			value.firstInteractionLedger,
			`${field}.firstInteractionLedger`
		),
		transactionCount: nullableNonNegativeInteger(
			value.transactionCount,
			`${field}.transactionCount`
		),
		successfulTransactionCount: nullableNonNegativeInteger(
			value.successfulTransactionCount,
			`${field}.successfulTransactionCount`
		),
		contracts: arrayValue(value.contracts, `${field}.contracts`).map((contract, contractIndex) =>
			contractAddress(contract, `${field}.contracts[${contractIndex}]`)
		)
	};
}

function parseContractPage(input: unknown): LiveContractPage {
	const value = objectValue(input, 'contracts');
	return {
		items: arrayValue(value.items, 'contracts.items').map(parseContractRelationship),
		limit: nonNegativeInteger(value.limit, 'contracts.limit'),
		nextCursor:
			value.nextCursor === null
				? null
				: contractAddress(value.nextCursor, 'contracts.nextCursor')
	};
}

function parseContractRelationship(input: unknown, index: number): LiveContractRelationship {
	const field = `contracts.items[${index}]`;
	const value = objectValue(input, field);
	const parsedContractAddress = contractAddress(
		value.contractAddress,
		`${field}.contractAddress`
	);
	const protocols = arrayValue(value.protocols, `${field}.protocols`).map(
		(protocol, protocolIndex) => {
			const protocolField = `${field}.protocols[${protocolIndex}]`;
			const protocolValue = objectValue(protocol, protocolField);
			return {
				protocolId: stringValue(protocolValue.protocolId, `${protocolField}.protocolId`),
				role: stringValue(protocolValue.role, `${protocolField}.role`),
				sourceUrl: nullableUrl(protocolValue.sourceUrl, `${protocolField}.sourceUrl`)
			};
		}
	);
	const attributionStatus = contractAttributionStatus(
		value.attributionStatus,
		`${field}.attributionStatus`
	);
	const placeholderProtocolId =
		value.placeholderProtocolId === null
			? null
			: contractAddress(value.placeholderProtocolId, `${field}.placeholderProtocolId`);
	const relationship = {
		contractAddress: parsedContractAddress,
		firstInteractionLedger: nonNegativeInteger(
			value.firstInteractionLedger,
			`${field}.firstInteractionLedger`
		),
		firstInteractionAt: nullableTimestamp(value.firstInteractionAt, `${field}.firstInteractionAt`),
		roleMask: nonNegativeInteger(value.roleMask, `${field}.roleMask`)
	};
	if (attributionStatus === 'verified-registry') {
		if (protocols.length === 0) {
			throw new Error(`${field} verified registry attribution requires at least one protocol`);
		}
		if (placeholderProtocolId !== null) {
			throw new Error(`${field} verified registry attribution cannot have a placeholder`);
		}
		return {
			...relationship,
			attributionStatus,
			placeholderProtocolId: null,
			protocols
		};
	}
	if (protocols.length !== 0) {
		throw new Error(`${field} unattributed contract cannot claim a protocol`);
	}
	if (placeholderProtocolId !== parsedContractAddress) {
		throw new Error(`${field} unattributed placeholder must equal contractAddress`);
	}
	return {
		...relationship,
		attributionStatus,
		placeholderProtocolId: parsedContractAddress,
		protocols: []
	};
}

function parseAlerts(input: unknown): LiveWalletAlerts {
	const value = objectValue(input, 'alerts');
	const available = booleanValue(value.available, 'alerts.available');
	const items = arrayValue(value.items, 'alerts.items').map(parseHistoricalSecurityFinding);
	const deliveryAvailable = booleanValue(value.deliveryAvailable, 'alerts.deliveryAvailable');

	let coverage: LiveWalletAlertCoverage | null = null;
	if (value.coverage !== null) {
		const coverageValue = objectValue(value.coverage, 'alerts.coverage');
		const fromLedger = nonNegativeInteger(
			coverageValue.fromLedger,
			'alerts.coverage.fromLedger'
		);
		const toLedger = nonNegativeInteger(coverageValue.toLedger, 'alerts.coverage.toLedger');
		if (fromLedger > toLedger) {
			throw new Error('alerts.coverage.fromLedger must not exceed alerts.coverage.toLedger');
		}
		coverage = { fromLedger, toLedger };
	}
	if (!available && (items.length !== 0 || coverage !== null || deliveryAvailable)) {
		throw new Error(
			'unavailable historical findings must have no coverage, items, or delivery capability'
		);
	}
	if (available && coverage === null) {
		throw new Error('alerts.coverage is required when historical findings are available');
	}
	if (deliveryAvailable) {
		throw new Error('alerts.deliveryAvailable must be false for historical replay');
	}
	if (
		coverage &&
		items.some(
			(item) =>
				item.ledgerSequence < coverage.fromLedger || item.ledgerSequence > coverage.toLedger
		)
	) {
		throw new Error('alerts.items contains a finding outside the stated replay coverage');
	}
	if (new Set(items.map((item) => item.id)).size !== items.length) {
		throw new Error('alerts.items contains duplicate finding IDs');
	}

	return {
		available,
		reason: stringValue(value.reason, 'alerts.reason'),
		deliveryAvailable,
		deliveryReason: stringValue(value.deliveryReason, 'alerts.deliveryReason'),
		coverage,
		items
	};
}

function parseCapabilities(input: unknown): LiveDashboardCapabilities {
	const value = objectValue(input, 'capabilities');
	if (value.protocolAttribution !== 'seed-roots-only') {
		throw new Error('capabilities.protocolAttribution must be "seed-roots-only"');
	}
	return {
		walletContractRelationships: booleanValue(
			value.walletContractRelationships,
			'capabilities.walletContractRelationships'
		),
		protocolAttribution: 'seed-roots-only',
		unattributedContractPlaceholders: booleanValue(
			value.unattributedContractPlaceholders,
			'capabilities.unattributedContractPlaceholders'
		),
		walletTransactionCounts: booleanValue(
			value.walletTransactionCounts,
			'capabilities.walletTransactionCounts'
		),
		walletLastActivity: booleanValue(
			value.walletLastActivity,
			'capabilities.walletLastActivity'
		),
		alerts: booleanValue(value.alerts, 'capabilities.alerts'),
		historicalFindings: booleanValue(
			value.historicalFindings,
			'capabilities.historicalFindings'
		),
		alertDelivery: booleanValue(value.alertDelivery, 'capabilities.alertDelivery'),
		leaderboard: booleanValue(value.leaderboard, 'capabilities.leaderboard')
	};
}

function parseLeaderboardRow(input: unknown, index: number): LiveLeaderboardRow {
	const field = `rows[${index}]`;
	const value = objectValue(input, field);
	return {
		rank: nonNegativeInteger(value.rank, `${field}.rank`),
		accountAddress: accountAddress(value.accountAddress, `${field}.accountAddress`),
		transactionCount: nonNegativeInteger(value.transactionCount, `${field}.transactionCount`),
		successfulTransactionCount: nonNegativeInteger(
			value.successfulTransactionCount,
			`${field}.successfulTransactionCount`
		),
		protocolCount: nonNegativeInteger(value.protocolCount, `${field}.protocolCount`),
		contractCount: nonNegativeInteger(value.contractCount, `${field}.contractCount`),
		lastInteractionAt: timestampValue(value.lastInteractionAt, `${field}.lastInteractionAt`)
	};
}

function parseHistoricalSecurityFinding(
	input: unknown,
	index: number
): HistoricalSecurityFinding {
	const field = `alerts.items[${index}]`;
	const value = objectValue(input, field);
	if (value.kind !== 'historical-security-incident') {
		throw new Error(`${field}.kind must be "historical-security-incident"`);
	}
	if (value.origin !== 'historical-replay') {
		throw new Error(`${field}.origin must be "historical-replay"`);
	}
	if (
		value.matchType !== 'contract-relationship' &&
		value.matchType !== 'verified-incident-participant'
	) {
		throw new Error(`${field}.matchType is invalid`);
	}
	const deliveries = arrayValue(value.deliveries, `${field}.deliveries`);
	if (deliveries.length !== 0) {
		throw new Error(`${field}.deliveries must be empty for historical replay findings`);
	}
	const sources = arrayValue(value.sources, `${field}.sources`).map((source, sourceIndex) => {
		const sourceField = `${field}.sources[${sourceIndex}]`;
		const sourceValue = objectValue(source, sourceField);
		return {
			label: stringValue(sourceValue.label, `${sourceField}.label`),
			url: httpsUrl(sourceValue.url, `${sourceField}.url`)
		};
	});
	if (sources.length === 0) {
		throw new Error(`${field}.sources must contain at least one evidence source`);
	}
	return {
		id: stringValue(value.id, `${field}.id`),
		kind: 'historical-security-incident',
		severity: alertSeverity(value.severity, `${field}.severity`),
		title: stringValue(value.title, `${field}.title`),
		summary: stringValue(value.summary, `${field}.summary`),
		incidentAt: timestampValue(value.incidentAt, `${field}.incidentAt`),
		ledgerSequence: nonNegativeInteger(value.ledgerSequence, `${field}.ledgerSequence`),
		protocolId: stringValue(value.protocolId, `${field}.protocolId`),
		protocolName: stringValue(value.protocolName, `${field}.protocolName`),
		contractAddress: contractAddress(value.contractAddress, `${field}.contractAddress`),
		transactionHash: transactionHash(value.transactionHash, `${field}.transactionHash`),
		origin: 'historical-replay',
		matchType: value.matchType,
		sources,
		deliveries: []
	};
}

function alertSeverity(input: unknown, field: string): AlertSeverity {
	if (input !== 'info' && input !== 'low' && input !== 'medium' && input !== 'high' && input !== 'critical') {
		throw new Error(`${field} is invalid`);
	}
	return input;
}

function leaderboardMetric(input: unknown, field: string): LeaderboardMetric {
	if (input !== 'transactions' && input !== 'protocols') {
		throw new Error(`${field} must be "transactions" or "protocols"`);
	}
	return input;
}

function contractAttributionStatus(
	input: unknown,
	field: string
): LiveContractAttributionStatus {
	if (input !== 'verified-registry' && input !== 'unattributed') {
		throw new Error(`${field} must be "verified-registry" or "unattributed"`);
	}
	return input;
}

function contractIndexAttribution(input: unknown, field: string): LiveContractIndexAttribution {
	if (input !== 'all' && input !== 'unattributed') {
		throw new Error(`${field} must be "all" or "unattributed"`);
	}
	return input;
}

function accountAddress(input: unknown, field: string): StellarAccountAddress {
	const value = stringValue(input, field);
	const parsed = parseStellarWalletAddress(value);
	if (!parsed.ok || parsed.value.kind !== 'account') {
		throw new Error(`${field} is not a checksum-valid Stellar G-address`);
	}
	return parsed.value.address;
}

function contractAddress(input: unknown, field: string): StellarContractAddress {
	const value = stringValue(input, field);
	if (!/^C[A-Z2-7]{55}$/.test(value)) {
		throw new Error(`${field} is not a Stellar C-address`);
	}
	return value as StellarContractAddress;
}

function transactionHash(input: unknown, field: string): StellarTransactionHash {
	const value = stringValue(input, field);
	if (!/^[0-9a-f]{64}$/.test(value)) {
		throw new Error(`${field} is not a lowercase 32-byte transaction hash`);
	}
	return value as StellarTransactionHash;
}

function nullableHash(input: unknown, field: string): string | null {
	if (input === null) return null;
	const value = stringValue(input, field);
	if (!/^[0-9a-f]{64}$/.test(value)) {
		throw new Error(`${field} is not a lowercase 32-byte hash`);
	}
	return value;
}

function timestampValue(input: unknown, field: string): string {
	const value = stringValue(input, field);
	if (!Number.isFinite(Date.parse(value))) throw new Error(`${field} is not a valid timestamp`);
	return value;
}

function nullableTimestamp(input: unknown, field: string): string | null {
	return input === null ? null : timestampValue(input, field);
}

function nullableUrl(input: unknown, field: string): string | null {
	if (input === null) return null;
	const value = stringValue(input, field);
	let parsed: URL;
	try {
		parsed = new URL(value);
	} catch {
		throw new Error(`${field} is not a valid URL`);
	}
	if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
		throw new Error(`${field} must use HTTP or HTTPS`);
	}
	return value;
}

function httpsUrl(input: unknown, field: string): string {
	const value = nullableUrl(input, field);
	if (value === null || !value.startsWith('https://')) {
		throw new Error(`${field} must use HTTPS`);
	}
	return value;
}

function nullableDecimalString(input: unknown, field: string): string | null {
	if (input === null) return null;
	return decimalString(input, field);
}

function decimalString(input: unknown, field: string): string {
	const value = stringValue(input, field);
	if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} is not an unsigned decimal ID`);
	return value;
}

function jsonObjectValue(input: unknown, field: string): Readonly<Record<string, LiveJsonValue>> {
	const value = objectValue(input, field);
	return Object.fromEntries(
		Object.entries(value).map(([key, item]) => [key, jsonValue(item, `${field}.${key}`, 1)])
	);
}

function jsonValue(input: unknown, field: string, depth: number): LiveJsonValue {
	if (depth > 8) throw new Error(`${field} exceeds the supported JSON depth`);
	if (input === null || typeof input === 'boolean' || typeof input === 'string') return input;
	if (typeof input === 'number') {
		if (!Number.isFinite(input)) throw new Error(`${field} must be a finite number`);
		return input;
	}
	if (Array.isArray(input)) {
		if (input.length > 256) throw new Error(`${field} contains too many values`);
		return input.map((item, index) => jsonValue(item, `${field}[${index}]`, depth + 1));
	}
	const value = objectValue(input, field);
	if (Object.keys(value).length > 256) throw new Error(`${field} contains too many fields`);
	return Object.fromEntries(
		Object.entries(value).map(([key, item]) => [key, jsonValue(item, `${field}.${key}`, depth + 1)])
	);
}

function objectValue(input: unknown, field: string): Record<string, unknown> {
	if (!isObject(input)) throw new Error(`${field} must be an object`);
	return input;
}

function arrayValue(input: unknown, field: string): readonly unknown[] {
	if (!Array.isArray(input)) throw new Error(`${field} must be an array`);
	return input;
}

function stringValue(input: unknown, field: string): string {
	if (typeof input !== 'string' || input.length === 0) throw new Error(`${field} must be a non-empty string`);
	return input;
}

function optionalString(input: unknown): string | undefined {
	return typeof input === 'string' ? input : undefined;
}

function booleanValue(input: unknown, field: string): boolean {
	if (typeof input !== 'boolean') throw new Error(`${field} must be a boolean`);
	return input;
}

function nonNegativeInteger(input: unknown, field: string): number {
	if (!Number.isSafeInteger(input) || (input as number) < 0) {
		throw new Error(`${field} must be a non-negative safe integer`);
	}
	return input as number;
}

function byteValue(input: unknown, field: string): number {
	const value = nonNegativeInteger(input, field);
	if (value > 255) throw new Error(`${field} must fit in one byte`);
	return value;
}

function nullableNonNegativeInteger(input: unknown, field: string): number | null {
	return input === null ? null : nonNegativeInteger(input, field);
}

function optionalNonNegativeInteger(input: unknown, field: string): number | undefined {
	return input === undefined ? undefined : nonNegativeInteger(input, field);
}

function isObject(input: unknown): input is Record<string, unknown> {
	return typeof input === 'object' && input !== null && !Array.isArray(input);
}

function errorMessage(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}
