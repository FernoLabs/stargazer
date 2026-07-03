import { error, redirect } from '@sveltejs/kit';
import { nextPageOffset } from '$lib/api-pagination';
import type { EventsResponse, DefiProtocolDescriptor } from '$lib/types';
import { canonicalProtocolSlug, syntheticSquadsDescriptor } from '$lib/catalog';
import { backendRequest, readBackendError } from '$lib/server/backend';
import type { PageServerLoad } from './$types';

export const load = (async ({ fetch, params, parent }) => {
	const slug = params.slug;
	const canonicalSlug = canonicalProtocolSlug(slug);
	if (canonicalSlug !== slug) {
		throw redirect(308, `/protocols/${canonicalSlug}`);
	}

	const app = await parent();
	const protocol =
		(app.defiProtocols.find((candidate) => candidate.slug === slug) as
			| DefiProtocolDescriptor
			| undefined) ?? (slug === 'squads' ? syntheticSquadsDescriptor() : undefined);

	if (!protocol) {
		throw error(404, 'Protocol not found');
	}

	let events: EventsResponse['items'] = [];
	let nextOffset: number | null = null;
	let protocolEventTotalCount = 0;
	let protocolAlertCounts = {
		critical: 0,
		high: 0
	};
	let protocolEventsError: string | null = null;

	try {
		const search = new URLSearchParams({ 'page[size]': '40' });
		search.set('protocol', protocol.slug);
		search.set('include_related_multisigs', 'true');

		const eventsResponse = await backendRequest(fetch, `/api/events?${search.toString()}`);
		if (!eventsResponse.ok) throw new Error(await readBackendError(eventsResponse));

		const payload = (await eventsResponse.json()) as EventsResponse;
		events = payload.items;
		nextOffset = nextPageOffset(payload.links);
		protocolEventTotalCount = payload.total_count ?? payload.items.length;
		protocolAlertCounts = {
			critical: payload.scope_counts?.critical ?? 0,
			high: payload.scope_counts?.high ?? 0
		};
	} catch (err) {
		protocolEventsError = err instanceof Error ? err.message : 'Unknown backend error';
	}

	return {
		protocol,
		events,
		nextOffset,
		protocolEventTotalCount,
		protocolAlertCounts,
		protocolEventsError
	};
}) satisfies PageServerLoad;
