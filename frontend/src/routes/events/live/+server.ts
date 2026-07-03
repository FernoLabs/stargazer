import { backendRequest, readBackendError } from '$lib/server/backend';
import type { RequestHandler } from './$types';

const FORWARDED_EVENT_PARAMS = [
	'page[size]',
	'page[offset]',
	'protocol',
	'include_related_multisigs',
	'target_address',
	'scope',
	'search',
	'severity',
	'coverage',
	'tag',
	'from_ts',
	'before_ts'
] as const;

export const GET = (async ({ fetch, url }) => {
	const search = new URLSearchParams();

	for (const key of FORWARDED_EVENT_PARAMS) {
		const value = url.searchParams.get(key);
		if (value != null) search.set(key, value);
	}

	const response = await backendRequest(fetch, `/api/events?${search.toString()}`);
	if (!response.ok) {
		return new Response(JSON.stringify({ error: await readBackendError(response) }), {
			status: response.status,
			headers: {
				'cache-control': 'no-store',
				'content-type': 'application/json'
			}
		});
	}

	return new Response(response.body, {
		status: response.status,
		statusText: response.statusText,
		headers: {
			'cache-control': 'no-store',
			'content-type': response.headers.get('content-type') ?? 'application/json'
		}
	});
}) satisfies RequestHandler;
