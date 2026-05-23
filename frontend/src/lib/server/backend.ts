import { env as privateEnv } from '$env/dynamic/private';
import { env as publicEnv } from '$env/dynamic/public';
import type {
	DefiProtocolDescriptor,
	DefiProtocolsResponse,
	LiveStatusResponse,
	TelegramSubscriptionsResponse
} from '$lib/types';

export const DEFAULT_BACKEND_ORIGIN_URL = 'http://127.0.0.1:8080';
export const DEFAULT_PUBLIC_BACKEND_BASE_URL = '/api/backend';
export const DEFAULT_EXPLORER_CLUSTER = 'mainnet-beta';

export interface AppShellLoadData {
	backendBaseUrl: string;
	backendError: string | null;
	explorerCluster: string;
	liveStatus: LiveStatusResponse | null;
	defiProtocols: DefiProtocolDescriptor[];
	telegramEnabled: boolean;
	telegramSubscriptions: TelegramSubscriptionsResponse['items'];
	telegramSubscriptionCount: number;
	protocolCount: number;
	trackedProgramCount: number;
	idlTrackedProgramCount: number;
	deepDecodeProtocolCount: number;
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

export async function loadAppShell(fetch: typeof globalThis.fetch): Promise<AppShellLoadData> {
	const backendBaseUrl = publicBackendBaseUrl();
	const explorerCluster =
		privateEnv.SOLANA_EXPLORER_CLUSTER ??
		publicEnv.PUBLIC_SOLANA_EXPLORER_CLUSTER ??
		DEFAULT_EXPLORER_CLUSTER;

	let liveStatus: LiveStatusResponse | null = null;
	let defiProtocols: DefiProtocolDescriptor[] = [];
	let telegramEnabled = false;
	let telegramSubscriptions: TelegramSubscriptionsResponse['items'] = [];
	let backendError: string | null = null;

	try {
		const [statusResponse, protocolsResponse, telegramResponse] = await Promise.all([
			backendRequest(fetch, '/api/live-status'),
			backendRequest(fetch, '/api/protocols'),
			backendRequest(fetch, '/api/telegram-subscriptions')
		]);

		if (!statusResponse.ok) throw new Error(await readBackendError(statusResponse));
		if (!protocolsResponse.ok) throw new Error(await readBackendError(protocolsResponse));
		if (!telegramResponse.ok) throw new Error(await readBackendError(telegramResponse));

		liveStatus = (await statusResponse.json()) as LiveStatusResponse;
		defiProtocols = ((await protocolsResponse.json()) as DefiProtocolsResponse).items;
		const telegramPayload = (await telegramResponse.json()) as TelegramSubscriptionsResponse;
		telegramSubscriptions = telegramPayload.items;
		telegramEnabled = liveStatus.telegram_enabled && telegramPayload.telegram_enabled;
	} catch (error) {
		backendError = error instanceof Error ? error.message : 'Unknown backend error';
	}

	return {
		backendBaseUrl,
		backendError,
		explorerCluster,
		liveStatus,
		defiProtocols,
		telegramEnabled,
		telegramSubscriptions,
		telegramSubscriptionCount: telegramSubscriptions.length,
		protocolCount: defiProtocols.length,
		trackedProgramCount: defiProtocols.reduce((count, protocol) => count + protocol.programs.length, 0),
		idlTrackedProgramCount: defiProtocols.reduce(
			(count, protocol) =>
				count + protocol.programs.filter((program) => program.idl_metadata_address.length > 0).length,
			0
		),
		deepDecodeProtocolCount: defiProtocols.filter((protocol) => protocol.coverage === 'deep_decode').length
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
