const NAS_ORIGIN = 'http://localhost:8787';

const STATIC_PATHS = new Set(['/health', '/api/protocols']);
const PREFIX_PATHS = ['/api/wallets/', '/api/contracts/unattributed', '/api/examples/wallets', '/api/leaderboard'];

function isAllowedPath(pathname: string): boolean {
	return STATIC_PATHS.has(pathname) || PREFIX_PATHS.some((prefix) => pathname.startsWith(prefix));
}

export default {
	async fetch(request, env): Promise<Response> {
		if (request.method !== 'GET' && request.method !== 'HEAD') {
			return new Response('Method not allowed', {
				status: 405,
				headers: { allow: 'GET, HEAD' }
			});
		}

		const requestUrl = new URL(request.url);
		if (!isAllowedPath(requestUrl.pathname)) {
			return new Response('Not found', { status: 404 });
		}

		const target = new URL(`${requestUrl.pathname}${requestUrl.search}`, NAS_ORIGIN);
		const headers = new Headers();
		headers.set('accept', request.headers.get('accept') ?? 'application/json');
		const requestId = request.headers.get('cf-ray');
		if (requestId) headers.set('x-stargazer-request-id', requestId);

		try {
			return await env.STARGAZER_NAS.fetch(
				new Request(target, {
					method: request.method,
					headers,
					signal: request.signal
				})
			);
		} catch (error) {
			console.error(
				JSON.stringify({
					event: 'stargazer_nas_request_failed',
					path: requestUrl.pathname,
					error: error instanceof Error ? error.message : String(error)
				})
			);
			return Response.json({ error: 'Stargazer data service unavailable' }, { status: 502 });
		}
	}
} satisfies ExportedHandler<Env>;
