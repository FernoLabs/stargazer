import { backendUrl } from '$lib/server/backend';
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
	'authorization',
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

function cachePolicyFor(path: string): CachePolicy | null {
	if (path === 'api/live-status') {
		return {
			header: 'public, max-age=1, s-maxage=2, stale-while-revalidate=5',
			edgeTtlSeconds: 2
		};
	}

	if (path === 'api/events') {
		return {
			header: 'public, max-age=0, s-maxage=3, stale-while-revalidate=15',
			edgeTtlSeconds: 3
		};
	}

	if (path === 'api/protocols') {
		return {
			header: 'public, max-age=300, s-maxage=3600, stale-while-revalidate=86400',
			edgeTtlSeconds: 3600
		};
	}

	if (/^api\/programs\/[^/]+\/signer-stats$/.test(path)) {
		return {
			header: 'public, max-age=60, s-maxage=300, stale-while-revalidate=3600',
			edgeTtlSeconds: 300
		};
	}

	if (/^api\/programs\/[^/]+\/(versions|idls)$/.test(path)) {
		return {
			header: 'public, max-age=60, s-maxage=300, stale-while-revalidate=3600',
			edgeTtlSeconds: 300
		};
	}

	if (/^api\/programs\/[^/]+\/(versions|idls)\/[^/]+\/(bytes|text|disassembly)$/.test(path)) {
		return {
			header: 'public, max-age=31536000, immutable',
			edgeTtlSeconds: 31536000
		};
	}

	return null;
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

	const method = event.request.method.toUpperCase();
	const cachePolicy = method === 'GET' ? cachePolicyFor(normalizedPath) : null;
	const cache = cachePolicy ? cacheForPlatform(event.platform) : undefined;
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
		headers: forwardHeaders(event.request),
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
