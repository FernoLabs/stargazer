import { error } from '@sveltejs/kit';
import type { EventsResponse, DefiProtocolDescriptor } from '$lib/types';
import { syntheticSquadsDescriptor } from '$lib/catalog';
import { backendRequest, readBackendError } from '$lib/server/backend';
import type { PageServerLoad } from './$types';

export const load = (async ({ fetch, params, parent }) => {
	const app = await parent();
	const slug = params.slug;
	const protocol =
		(app.defiProtocols.find((candidate) => candidate.slug === slug) as
			| DefiProtocolDescriptor
			| undefined) ?? (slug === 'squads' ? syntheticSquadsDescriptor() : undefined);

	if (!protocol) {
		throw error(404, 'Protocol not found');
	}

	let events: EventsResponse['items'] = [];
	let nextOffset: number | null = null;
	let protocolEventsError: string | null = null;

	try {
		const search = new URLSearchParams({ limit: '120' });
		search.set('protocol', protocol.slug);
		search.set('include_related_multisigs', 'true');

		const eventsResponse = await backendRequest(fetch, `/api/events?${search.toString()}`);
		if (!eventsResponse.ok) throw new Error(await readBackendError(eventsResponse));

		const payload = (await eventsResponse.json()) as EventsResponse;
		events = payload.items;
		nextOffset = payload.next_offset;
	} catch (err) {
		protocolEventsError = err instanceof Error ? err.message : 'Unknown backend error';
	}

	return {
		protocol,
		events,
		nextOffset,
		protocolEventsError
	};
}) satisfies PageServerLoad;
