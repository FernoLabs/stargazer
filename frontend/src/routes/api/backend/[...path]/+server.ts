import { adminUnauthorizedResponse, verifyAdminAuthorization } from '$lib/server/admin-auth';
import { backendOriginHeaders, backendUrl } from '$lib/server/backend';
import type { RequestHandler } from './$types';

type EdgePlatform = {
	caches?: { default?: Cache };
	context?: { waitUntil(promise: Promise<unknown>): void };
};

type CachePolicy = {
	header: string;
	edgeTtlSeconds: number;
};

const HOP_BY_HOP_HEADERS = new Set([
	'connection',
	'keep-alive',
	'proxy-authenticate',
	'proxy-authorization',
	'te',
	'trailer',
	'transfer-encoding',
	'upgrade'
]);
const FORWARDED_HEADERS = new Set([
	'accept',
	'accept-language',
	'content-type',
	'if-modified-since',
	'if-none-match',
	'range'
]);

function normalizeBackendPath(path: string | undefined) {
	const normalizedPath = (path ?? '').replace(/^\/+/, '');
	if (normalizedPath === 'healthz' || normalizedPath.startsWith('api/')) return normalizedPath;
	return null;
}

function cachePolicyFor(path: string, searchParams: URLSearchParams): CachePolicy | null {
	if (path === 'api/live-status') {
		return {
			header: 'public, max-age=1, s-maxage=2, stale-while-revalidate=5',
			edgeTtlSeconds: 2
		};
	}

	if (path === 'api/status') {
		return {
			header: 'public, max-age=5, s-maxage=10, stale-while-revalidate=30',
			edgeTtlSeconds: 10
		};
	}

	if (path === 'api/events') {
		const offset = Number.parseInt(searchParams.get('page[offset]') ?? '0', 10);
		const hasFocusedFilter = [
			'protocol',
			'target_address',
			'scope',
			'severity',
			'coverage',
			'tag',
			'search'
		].some((key) => searchParams.has(key));

		if (offset > 0 || hasFocusedFilter) {
			return {
				header: 'public, max-age=10, s-maxage=60, stale-while-revalidate=300',
				edgeTtlSeconds: 60
			};
		}

		return {
			header: 'public, max-age=5, s-maxage=15, stale-while-revalidate=60',
			edgeTtlSeconds: 15
		};
	}

	if (/^api\/events\/[^/]+$/.test(path)) {
		return {
			header: 'public, max-age=300, s-maxage=3600, stale-while-revalidate=86400',
			edgeTtlSeconds: 3600
		};
	}

	if (/^api\/events\/[^/]+\/preview-card\.png$/.test(path)) {
		return {
			header: 'public, max-age=300, s-maxage=86400, stale-while-revalidate=604800',
			edgeTtlSeconds: 86400
		};
	}

	if (path === 'api/protocols') {
		return {
			header: 'public, max-age=60, s-maxage=300, stale-while-revalidate=1800',
			edgeTtlSeconds: 300
		};
	}

	if (/^api\/programs\/[^/]+\/signer-stats$/.test(path)) {
		return {
			header: 'public, max-age=300, s-maxage=1800, stale-while-revalidate=21600',
			edgeTtlSeconds: 1800
		};
	}

	if (path === 'api/programs/version-summaries') {
		return {
			header: 'public, max-age=300, s-maxage=1800, stale-while-revalidate=21600',
			edgeTtlSeconds: 1800
		};
	}

	if (/^api\/programs\/[^/]+\/(versions|idls)$/.test(path)) {
		return {
			header: 'public, max-age=300, s-maxage=3600, stale-while-revalidate=86400',
			edgeTtlSeconds: 3600
		};
	}

	if (/^api\/programs\/[^/]+\/(versions|idls)\/[^/]+\/(bytes|text)$/.test(path)) {
		return {
			header: 'public, max-age=31536000, immutable',
			edgeTtlSeconds: 31536000
		};
	}

	if (/^api\/programs\/[^/]+\/versions\/[^/]+\/disassembly$/.test(path)) {
		return {
			header: 'public, max-age=31536000, immutable',
			edgeTtlSeconds: 31536000
		};
	}

	return null;
}

function isProtectedBackendPath(path: string) {
	return (
		path === 'api/metric-alerts' ||
		path === 'api/metric-alert-rules' ||
		path.startsWith('api/metric-alert-rules/') ||
		path === 'api/promo-codes' ||
		path.startsWith('api/promo-codes/') ||
		path === 'api/users' ||
		path.startsWith('api/users/') ||
		path === 'api/payment-links' ||
		path === 'api/admin/storage' ||
		path === 'api/admin/storage/migrate-blobs' ||
		path === 'api/telegram-subscriptions' ||
		path.startsWith('api/telegram-subscriptions/')
	);
}

function cacheForPlatform(platform: App.Platform | undefined) {
	const edgePlatform = platform as EdgePlatform | undefined;
	return edgePlatform?.caches?.default;
}

function forwardHeaders(request: Request) {
	const headers = new Headers();
	for (const [key, value] of request.headers) {
		const lowerKey = key.toLowerCase();
		if (HOP_BY_HOP_HEADERS.has(lowerKey)) continue;
		if (!FORWARDED_HEADERS.has(lowerKey)) continue;
		headers.set(key, value);
	}
	return headers;
}

function responseHeaders(originHeaders: Headers, policy: CachePolicy | null) {
	const headers = new Headers(originHeaders);
	for (const header of HOP_BY_HOP_HEADERS) headers.delete(header);

	if (policy) {
		headers.set('cache-control', policy.header);
	} else {
		headers.set('cache-control', 'no-store');
	}

	return headers;
}

const proxy: RequestHandler = async (event) => {
	const normalizedPath = normalizeBackendPath(event.params.path);
	if (!normalizedPath) {
		return new Response('Not found', { status: 404 });
	}

	if (isProtectedBackendPath(normalizedPath)) {
		const authorization = await verifyAdminAuthorization(event.fetch, event.request);
		if (!authorization.authorized) {
			return adminUnauthorizedResponse(authorization);
		}
	}

	const method = event.request.method.toUpperCase();
	const isReadRequest = method === 'GET' || method === 'HEAD';
	const cachePolicy = isReadRequest ? cachePolicyFor(normalizedPath, event.url.searchParams) : null;
	const cache = method === 'GET' && cachePolicy ? cacheForPlatform(event.platform) : undefined;
	const cacheKey = cache ? new Request(event.url.toString(), { method: 'GET' }) : null;

	if (cache && cacheKey) {
		const cachedResponse = await cache.match(cacheKey);
		if (cachedResponse) return cachedResponse;
	}

	const targetUrl = `${backendUrl(`/${normalizedPath}`)}${event.url.search}`;
	const body =
		method === 'GET' || method === 'HEAD' ? undefined : await event.request.arrayBuffer();
	const originResponse = await event.fetch(targetUrl, {
		method,
		headers: backendOriginHeaders(forwardHeaders(event.request)),
		body
	});
	const response = new Response(originResponse.body, {
		status: originResponse.status,
		statusText: originResponse.statusText,
		headers: responseHeaders(originResponse.headers, cachePolicy)
	});

	if (cache && cacheKey && response.ok) {
		const edgePlatform = event.platform as EdgePlatform | undefined;
		const put = cache.put(cacheKey, response.clone());
		if (edgePlatform?.context) {
			edgePlatform.context.waitUntil(put);
		} else {
			await put;
		}
	}

	return response;
};

export const GET = proxy;
export const POST = proxy;
export const DELETE = proxy;
export const HEAD = proxy;
