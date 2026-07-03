import { backendRequest, readBackendError } from '$lib/server/backend';
import { canonicalProtocolSlug, normalizedProtocolCatalog } from '$lib/catalog';
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
		const defiProtocols = normalizedProtocolCatalog(payload.items);
		for (const protocol of defiProtocols) {
			protocolActivitySummaries[protocol.slug] = emptyActivitySummary('Activity unavailable');
		}
		for (const item of payload.included?.activity_summaries ?? []) {
			const slug = canonicalProtocolSlug(item.slug);
			const existing = protocolActivitySummaries[slug];
			protocolActivitySummaries[slug] = {
				total: (existing?.error ? 0 : existing?.total ?? 0) + item.total,
				critical: (existing?.error ? 0 : existing?.critical ?? 0) + item.critical,
				high: (existing?.error ? 0 : existing?.high ?? 0) + item.high,
				standard: (existing?.error ? 0 : existing?.standard ?? 0) + item.standard,
				error: null
			};
		}

		return {
			defiProtocols,
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
