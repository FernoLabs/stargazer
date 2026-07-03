import { json } from '@sveltejs/kit';
import { backendRequest, readBackendError } from '$lib/server/backend';
import type { EventsSummary, LiveStatusResponse } from '$lib/types';
import type { RequestHandler } from './$types';

const ESTIMATED_SLOT_TIME_MS = 400;

async function backendJson<T>(fetch: typeof globalThis.fetch, path: string) {
	const response = await backendRequest(fetch, path);
	if (!response.ok) throw new Error(await readBackendError(response));
	return (await response.json()) as T;
}

function noStoreHeaders() {
	return {
		'cache-control': 'no-store'
	};
}

export const GET = (async ({ fetch }) => {
	try {
		const payload = await backendJson<LiveStatusResponse>(fetch, '/api/live-status');
		return json(payload, { headers: noStoreHeaders() });
	} catch (liveStatusError) {
		try {
			const summary = await backendJson<EventsSummary>(fetch, '/api/status');
			return json(
				{
					summary,
					telegram_enabled: false,
					telegram_bot: null,
					sampled_at_ms: Date.now(),
					estimated_slot_time_ms: ESTIMATED_SLOT_TIME_MS
				} satisfies LiveStatusResponse,
				{ headers: noStoreHeaders() }
			);
		} catch (statusError) {
			const liveMessage =
				liveStatusError instanceof Error ? liveStatusError.message : 'Live status unavailable';
			const statusMessage = statusError instanceof Error ? statusError.message : 'Status unavailable';
			return json(
				{ error: `${liveMessage}; ${statusMessage}` },
				{ status: 502, headers: noStoreHeaders() }
			);
		}
	}
}) satisfies RequestHandler;
