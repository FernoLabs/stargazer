import type { StellarAccountAddress } from '$lib/dashboard/types';

export type AlertPairingStatus = 'pendingTelegram' | 'paired' | 'expired';
export type AlertPolicyState = 'pendingTelegram' | 'awaitingEntitlement' | 'active' | 'paused';
export type AlertBillingMode = 'unconfigured' | 'pilot';
export type AlertPaymentState = 'unconfigured' | 'notRequired';
export type AlertEntitlementState = 'notConfigured' | 'grantedOnPairing';

export interface CreateAlertPairingInput {
	readonly walletAddress: StellarAccountAddress;
	readonly criticalAlerts: boolean;
	readonly nonCriticalAlerts: boolean;
}

export interface AlertPairingPolicy {
	readonly criticalAlerts: boolean;
	readonly nonCriticalAlerts: boolean;
	readonly state: AlertPolicyState;
}

export interface AlertPairingBilling {
	readonly mode: AlertBillingMode;
	readonly paymentState: AlertPaymentState;
	readonly entitlementState: AlertEntitlementState;
}

export interface AlertPairingStatusResponse {
	readonly expiresAt: number;
	readonly status: AlertPairingStatus;
	readonly walletAddress: StellarAccountAddress;
	readonly policy: AlertPairingPolicy;
	readonly billing: AlertPairingBilling;
}

export interface CreateAlertPairingResponse extends AlertPairingStatusResponse {
	readonly claimToken: string;
	readonly telegramUrl: string;
}

export class AlertControlError extends Error {
	readonly code: string;
	readonly status: number | null;

	constructor(message: string, code = 'alert_control_error', status: number | null = null) {
		super(message);
		this.name = 'AlertControlError';
		this.code = code;
		this.status = status;
	}
}

export class AlertControlClient {
	readonly baseUrl: string;
	readonly #fetch: typeof fetch;

	constructor(baseUrl: string, fetcher: typeof fetch = globalThis.fetch) {
		this.baseUrl = normalizeBaseUrl(baseUrl);
		this.#fetch = fetcher;
	}

	async createPairing(input: CreateAlertPairingInput): Promise<CreateAlertPairingResponse> {
		const response = await this.#fetch(`${this.baseUrl}/api/alert-pairings`, {
			method: 'POST',
			headers: {
				accept: 'application/json',
				'content-type': 'application/json'
			},
			body: JSON.stringify(input),
			cache: 'no-store'
		});
		const payload = await readPayload(response);
		if (!response.ok) throw parseControlError(response.status, payload);
		return parseCreateResponse(payload);
	}

	async getPairingStatus(claimToken: string): Promise<AlertPairingStatusResponse> {
		if (!/^[a-f0-9]{64}$/.test(claimToken)) {
			throw new AlertControlError('The Telegram pairing token is invalid.', 'invalid_claim_token');
		}
		const response = await this.#fetch(`${this.baseUrl}/api/alert-pairings/status`, {
			method: 'GET',
			headers: {
				accept: 'application/json',
				authorization: `Bearer ${claimToken}`
			},
			cache: 'no-store'
		});
		const payload = await readPayload(response);
		if (!response.ok) throw parseControlError(response.status, payload);
		return parseStatusResponse(payload);
	}
}

function normalizeBaseUrl(input: string): string {
	let url: URL;
	try {
		url = new URL(input.trim());
	} catch (error) {
		throw new AlertControlError('The alert control service URL is invalid.', 'invalid_base_url');
	}
	if (url.protocol !== 'http:' && url.protocol !== 'https:') {
		throw new AlertControlError('The alert control service must use HTTP or HTTPS.', 'invalid_base_url');
	}
	if (url.username || url.password || url.search || url.hash) {
		throw new AlertControlError(
			'The alert control service URL cannot contain credentials, a query, or a fragment.',
			'invalid_base_url'
		);
	}
	return url.toString().replace(/\/$/, '');
}

async function readPayload(response: Response): Promise<unknown> {
	try {
		return await response.json();
	} catch (error) {
		throw new AlertControlError(
			'The alert control service returned an unreadable response.',
			'invalid_response',
			response.status
		);
	}
}

function parseCreateResponse(input: unknown): CreateAlertPairingResponse {
	const value = objectValue(input);
	const response = parseStatusResponse(value);
	const claimToken = stringValue(value.claimToken, 'claimToken');
	if (!/^[a-f0-9]{64}$/.test(claimToken)) throw new Error('claimToken is not a 64-character token');
	const telegramUrl = stringValue(value.telegramUrl, 'telegramUrl');
	const parsedTelegramUrl = new URL(telegramUrl);
	if (
		parsedTelegramUrl.protocol !== 'https:' ||
		parsedTelegramUrl.hostname !== 't.me' ||
		parsedTelegramUrl.searchParams.get('start') !== claimToken
	) {
		throw new Error('telegramUrl is not the expected HTTPS deep link');
	}
	return { ...response, claimToken, telegramUrl };
}

function parseStatusResponse(input: unknown): AlertPairingStatusResponse {
	try {
		const value = objectValue(input);
		const policy = objectValue(value.policy);
		const billing = objectValue(value.billing);
		return {
			expiresAt: integerValue(value.expiresAt, 'expiresAt'),
			status: enumValue(value.status, ['pendingTelegram', 'paired', 'expired'], 'status'),
			walletAddress: stellarAccountValue(value.walletAddress),
			policy: {
				criticalAlerts: booleanValue(policy.criticalAlerts, 'policy.criticalAlerts'),
				nonCriticalAlerts: booleanValue(policy.nonCriticalAlerts, 'policy.nonCriticalAlerts'),
				state: enumValue(
					policy.state,
					['pendingTelegram', 'awaitingEntitlement', 'active', 'paused'],
					'policy.state'
				)
			},
			billing: {
				mode: enumValue(billing.mode, ['unconfigured', 'pilot'], 'billing.mode'),
				paymentState: enumValue(
					billing.paymentState,
					['unconfigured', 'notRequired'],
					'billing.paymentState'
				),
				entitlementState: enumValue(
					billing.entitlementState,
					['notConfigured', 'grantedOnPairing'],
					'billing.entitlementState'
				)
			}
		};
	} catch (error) {
		if (error instanceof AlertControlError) throw error;
		throw new AlertControlError(
			'The alert control service returned an unexpected response.',
			'invalid_response'
		);
	}
}

function parseControlError(status: number, input: unknown): AlertControlError {
	if (isObject(input) && isObject(input.error)) {
		const code = typeof input.error.code === 'string' ? input.error.code : `http_${status}`;
		const message =
			typeof input.error.message === 'string'
				? input.error.message
				: `Alert control service returned HTTP ${status}.`;
		return new AlertControlError(message, code, status);
	}
	return new AlertControlError(`Alert control service returned HTTP ${status}.`, `http_${status}`, status);
}

function objectValue(input: unknown): Record<string, unknown> {
	if (!isObject(input)) throw new Error('expected an object');
	return input;
}

function stringValue(input: unknown, field: string): string {
	if (typeof input !== 'string' || input.length === 0) throw new Error(`${field} must be a string`);
	return input;
}

function booleanValue(input: unknown, field: string): boolean {
	if (typeof input !== 'boolean') throw new Error(`${field} must be a boolean`);
	return input;
}

function integerValue(input: unknown, field: string): number {
	if (!Number.isSafeInteger(input) || (input as number) < 0) {
		throw new Error(`${field} must be a non-negative integer`);
	}
	return input as number;
}

function enumValue<const Values extends readonly string[]>(
	input: unknown,
	values: Values,
	field: string
): Values[number] {
	if (typeof input !== 'string' || !values.includes(input)) {
		throw new Error(`${field} has an unsupported value`);
	}
	return input as Values[number];
}

function stellarAccountValue(input: unknown): StellarAccountAddress {
	if (typeof input !== 'string' || !/^G[A-Z2-7]{55}$/.test(input)) {
		throw new Error('walletAddress is not a canonical G-address');
	}
	return input as StellarAccountAddress;
}

function isObject(input: unknown): input is Record<string, unknown> {
	return typeof input === 'object' && input !== null && !Array.isArray(input);
}
