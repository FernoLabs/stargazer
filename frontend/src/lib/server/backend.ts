import { env as privateEnv } from '$env/dynamic/private';
import { env as publicEnv } from '$env/dynamic/public';
import type {
	DefiProtocolDescriptor,
	DefiProtocolsResponse,
	LiveStatusResponse,
	TelegramSubscriptionSummaryResponse
} from '$lib/types';
import { normalizedProtocolCatalog } from '$lib/catalog';
import { customPlanPricing, type CustomPlanPricing } from '$lib/server/custom-plan-pricing';

export const DEFAULT_BACKEND_ORIGIN_URL = 'http://127.0.0.1:8080';
export const DEFAULT_PUBLIC_BACKEND_BASE_URL = '/api/backend';
export const DEFAULT_EXPLORER_CLUSTER = 'mainnet-beta';
const APP_SHELL_FETCH_TIMEOUT_MS = 6000;
const LIVE_STATUS_CACHE_MS = 3000;
const PROTOCOLS_CACHE_MS = 5 * 60 * 1000;
const TELEGRAM_SUMMARY_CACHE_MS = 15_000;

export interface AppShellLoadData {
	backendBaseUrl: string;
	backendError: string | null;
	explorerCluster: string;
	liveStatus: LiveStatusResponse | null;
	defiProtocols: DefiProtocolDescriptor[];
	telegramEnabled: boolean;
	telegramSubscriptionCount: number;
	protocolCount: number;
	trackedProgramCount: number;
	idlTrackedProgramCount: number;
	deepDecodeProtocolCount: number;
	customPlanPricing: CustomPlanPricing;
}

type TimedCache<T> = {
	value: T;
	expiresAtMs: number;
};

let liveStatusCache: TimedCache<LiveStatusResponse> | null = null;
let protocolsCache: TimedCache<DefiProtocolDescriptor[]> | null = null;
let telegramSummaryCache: TimedCache<TelegramSubscriptionSummaryResponse> | null = null;

function cachedValue<T>(cache: TimedCache<T> | null) {
	return cache && cache.expiresAtMs > Date.now() ? cache.value : null;
}

function cacheUntil<T>(value: T, ttlMs: number): TimedCache<T> {
	return {
		value,
		expiresAtMs: Date.now() + ttlMs
	};
}

export function backendOriginBaseUrl() {
	return privateEnv.BACKEND_ORIGIN_URL ?? privateEnv.BACKEND_BASE_URL ?? DEFAULT_BACKEND_ORIGIN_URL;
}

export function publicBackendBaseUrl() {
	return (
		publicEnv.PUBLIC_BACKEND_BASE_URL ??
		privateEnv.PUBLIC_BACKEND_BASE_URL ??
		DEFAULT_PUBLIC_BACKEND_BASE_URL
	);
}

export function backendUrl(path: string) {
	const baseUrl = backendOriginBaseUrl().replace(/\/+$/, '');
	const normalizedPath = path.startsWith('/') ? path : `/${path}`;
	return `${baseUrl}${normalizedPath}`;
}

export function backendOriginHeaders(headers?: HeadersInit) {
	const nextHeaders = new Headers(headers);
	const originToken = privateEnv.BACKEND_ORIGIN_TOKEN || privateEnv.ORIGIN_API_TOKEN;
	if (originToken) {
		nextHeaders.set('x-firewatch-origin-token', originToken);
	}
	return nextHeaders;
}

export function publicBackendUrl(path: string) {
	const baseUrl = publicBackendBaseUrl().replace(/\/+$/, '');
	const normalizedPath = path.startsWith('/') ? path : `/${path}`;
	return `${baseUrl}${normalizedPath}`;
}

export function backendRequest(
	fetch: typeof globalThis.fetch,
	path: string,
	init?: RequestInit
) {
	return fetch(publicBackendUrl(path), init);
}

export function backendOriginRequest(
	fetch: typeof globalThis.fetch,
	path: string,
	init: RequestInit = {}
) {
	return fetch(backendUrl(path), {
		...init,
		headers: backendOriginHeaders(init.headers)
	});
}

async function withTimeout<T>(loader: (signal: AbortSignal) => Promise<T>, timeoutMs: number) {
	const controller = new AbortController();
	const timeout = setTimeout(() => controller.abort(), timeoutMs);

	try {
		return await loader(controller.signal);
	} finally {
		clearTimeout(timeout);
	}
}

async function backendJson<T>(
	fetch: typeof globalThis.fetch,
	path: string,
	options: { origin?: boolean; timeoutMs?: number } = {}
) {
	return withTimeout(async (signal) => {
		const response = options.origin
			? await backendOriginRequest(fetch, path, { signal })
			: await backendRequest(fetch, path, { signal });

		if (!response.ok) {
			throw new Error(await readBackendError(response));
		}

		return (await response.json()) as T;
	}, options.timeoutMs ?? APP_SHELL_FETCH_TIMEOUT_MS);
}

async function loadLiveStatus(fetch: typeof globalThis.fetch) {
	const cached = cachedValue(liveStatusCache);
	if (cached) return cached;

	try {
		const value = await backendJson<LiveStatusResponse>(fetch, '/api/live-status');
		liveStatusCache = cacheUntil(value, LIVE_STATUS_CACHE_MS);
		return value;
	} catch (error) {
		if (liveStatusCache) return liveStatusCache.value;
		throw error;
	}
}

async function loadDefiProtocols(fetch: typeof globalThis.fetch) {
	const cached = cachedValue(protocolsCache);
	if (cached) return cached;

	try {
		const value = normalizedProtocolCatalog(
			(await backendJson<DefiProtocolsResponse>(fetch, '/api/protocols')).items
		);
		protocolsCache = cacheUntil(value, PROTOCOLS_CACHE_MS);
		return value;
	} catch (error) {
		if (protocolsCache) return protocolsCache.value;
		throw error;
	}
}

async function loadTelegramSummary(fetch: typeof globalThis.fetch) {
	const cached = cachedValue(telegramSummaryCache);
	if (cached) return cached;

	try {
		const value = await backendJson<TelegramSubscriptionSummaryResponse>(
			fetch,
			'/api/telegram-subscriptions/summary',
			{ origin: true }
		);
		telegramSummaryCache = cacheUntil(value, TELEGRAM_SUMMARY_CACHE_MS);
		return value;
	} catch (error) {
		if (telegramSummaryCache) return telegramSummaryCache.value;
		throw error;
	}
}

function backendLoadError(label: string, reason: unknown) {
	const message = reason instanceof Error ? reason.message : 'Unknown backend error';
	return `${label}: ${message}`;
}

export async function loadAppShell(fetch: typeof globalThis.fetch): Promise<AppShellLoadData> {
	const backendBaseUrl = publicBackendBaseUrl();
	const explorerCluster =
		privateEnv.SOLANA_EXPLORER_CLUSTER ??
		publicEnv.PUBLIC_SOLANA_EXPLORER_CLUSTER ??
		DEFAULT_EXPLORER_CLUSTER;

	let liveStatus: LiveStatusResponse | null = null;
	let defiProtocols: DefiProtocolDescriptor[] = [];
	let telegramEnabled = false;
	let telegramSubscriptionCount = 0;
	let backendError: string | null = null;
	const backendErrors: string[] = [];

	const [statusResult, protocolsResult] = await Promise.allSettled([
		loadLiveStatus(fetch),
		loadDefiProtocols(fetch)
	]);

	if (statusResult.status === 'fulfilled') {
		liveStatus = statusResult.value;
	} else {
		backendErrors.push(backendLoadError('Live status', statusResult.reason));
	}

	if (protocolsResult.status === 'fulfilled') {
		defiProtocols = protocolsResult.value;
	} else {
		backendErrors.push(backendLoadError('Protocols', protocolsResult.reason));
	}

	if (liveStatus) {
		try {
			const telegramPayload = await loadTelegramSummary(fetch);
			telegramSubscriptionCount = telegramPayload.subscription_count;
			telegramEnabled = liveStatus.telegram_enabled && telegramPayload.telegram_enabled;
		} catch (error) {
			backendErrors.push(backendLoadError('Telegram summary', error));
			telegramSubscriptionCount = 0;
			telegramEnabled = false;
		}
	}

	backendError = backendErrors.length ? backendErrors.join(' ') : null;

	return {
		backendBaseUrl,
		backendError,
		explorerCluster,
		liveStatus,
		defiProtocols,
		telegramEnabled,
		telegramSubscriptionCount,
		protocolCount: defiProtocols.length,
		trackedProgramCount: defiProtocols.reduce((count, protocol) => count + protocol.programs.length, 0),
		idlTrackedProgramCount: defiProtocols.reduce(
			(count, protocol) =>
				count + protocol.programs.filter((program) => program.idl_metadata_address.length > 0).length,
			0
		),
		deepDecodeProtocolCount: defiProtocols.filter((protocol) => protocol.coverage === 'deep_decode').length,
		customPlanPricing: customPlanPricing()
	};
}

export async function readBackendError(response: Response) {
	try {
		const payload = await response.json();
		if (typeof payload?.error === 'string') {
			return payload.error;
		}
	} catch {
		// Ignore parse failures and fall through to the generic message.
	}

	return `Backend returned ${response.status}`;
}
