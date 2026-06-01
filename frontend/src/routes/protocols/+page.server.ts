import { backendRequest, readBackendError } from '$lib/server/backend';
import type { DefiProtocolsResponse, ProtocolActivitySummary } from '$lib/types';
import type { PageServerLoad } from './$types';

type ProtocolActivitySummaryView = Omit<ProtocolActivitySummary, 'slug'> & {
	error: string | null;
};

function emptyActivitySummary(error: string | null = null): ProtocolActivitySummaryView {
	return {
		total: 0,
		critical: 0,
		high: 0,
		standard: 0,
		error
	};
}

export const load = (async ({ fetch, parent }) => {
	const protocolActivitySummaries: Record<string, ProtocolActivitySummaryView> = {};

	try {
		const response = await backendRequest(fetch, '/api/protocols?include=activitySummary');
		if (!response.ok) throw new Error(await readBackendError(response));

		const payload = (await response.json()) as DefiProtocolsResponse;
		for (const protocol of payload.items) {
			protocolActivitySummaries[protocol.slug] = emptyActivitySummary('Activity unavailable');
		}
		for (const item of payload.included?.activity_summaries ?? []) {
			protocolActivitySummaries[item.slug] = {
				total: item.total,
				critical: item.critical,
				high: item.high,
				standard: item.standard,
				error: null
			};
		}

		return {
			defiProtocols: payload.items,
			protocolActivitySummaries
		};
	} catch (error) {
		const message = error instanceof Error ? error.message : 'Activity unavailable';
		const app = await parent();
		for (const protocol of app.defiProtocols) {
			protocolActivitySummaries[protocol.slug] = emptyActivitySummary(message);
		}

		return {
			defiProtocols: app.defiProtocols,
			protocolActivitySummaries
		};
	}
}) satisfies PageServerLoad;
