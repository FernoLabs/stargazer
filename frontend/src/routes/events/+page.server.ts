import type { EventsResponse } from '$lib/types';
import { nextPageOffset } from '$lib/api-pagination';
import { backendRequest, readBackendError } from '$lib/server/backend';
import type { PageServerLoad } from './$types';

const PAGE_SIZE = 40;
const EMPTY_SCOPE_COUNTS = {
	all: 0,
	critical: 0,
	high: 0,
	standard: 0,
	defi: 0,
	programs: 0,
	idl: 0,
	multisig: 0
};

type EventItem = EventsResponse['items'][number];

function isCriticalEvent(event: EventItem) {
	return event.tags.includes('severity:critical') || event.tags.includes('signal:unknown_instruction');
}

function isIdlEvent(event: EventItem) {
	return event.kind.startsWith('program_idl_');
}

function isProgramEvent(event: EventItem) {
	return (
		event.kind === 'program_deployed' ||
		event.kind === 'program_upgraded' ||
		event.kind === 'program_finalized' ||
		event.kind === 'program_migrated_to_loader_v4'
	);
}

function scopeCountsForItems(items: EventsResponse['items']): EventsResponse['scope_counts'] {
	return items.reduce(
		(counts, event) => {
				counts.all += 1;
				if (isCriticalEvent(event)) counts.critical += 1;
				if (event.tags.includes('severity:high')) counts.high += 1;
				if (event.tags.includes('severity:standard')) counts.standard += 1;
				if (event.tags.includes('watch:defi')) counts.defi += 1;
			if (isProgramEvent(event)) counts.programs += 1;
			if (isIdlEvent(event)) counts.idl += 1;
			if (event.target_type === 'multisig') counts.multisig += 1;
			return counts;
		},
		{ ...EMPTY_SCOPE_COUNTS }
	);
}

export const load = (async ({ fetch, url }) => {
	let events: EventsResponse['items'] = [];
	let summary: EventsResponse['summary'] | null = null;
	let totalCount = 0;
	let scopeCounts: EventsResponse['scope_counts'] = EMPTY_SCOPE_COUNTS;
	let nextOffset: number | null = null;
	let eventsBackendError: string | null = null;
	const protocol = url.searchParams.get('protocol')?.trim() || '';
	const target = url.searchParams.get('target')?.trim() || '';
	const scope = url.searchParams.get('scope')?.trim() || '';
	const searchText = url.searchParams.get('search')?.trim() || '';
	const severity = url.searchParams.get('severity')?.trim() || '';
	const coverage = url.searchParams.get('coverage')?.trim() || '';
	const tag = url.searchParams.get('tag')?.trim() || '';

	try {
		const search = new URLSearchParams({ 'page[size]': String(PAGE_SIZE), 'page[offset]': '0' });
		if (protocol) search.set('protocol', protocol);
		if (target) search.set('target_address', target);
		if (scope) search.set('scope', scope);
		if (searchText) search.set('search', searchText);
		if (severity) search.set('severity', severity);
		if (coverage) search.set('coverage', coverage);
		if (tag) search.set('tag', tag);
		if (protocol) search.set('include_related_multisigs', 'true');

		const eventsResponse = await backendRequest(fetch, `/api/events?${search.toString()}`);
		if (!eventsResponse.ok) throw new Error(await readBackendError(eventsResponse));

		const eventsPayload = (await eventsResponse.json()) as EventsResponse;
		events = eventsPayload.items;
		summary = eventsPayload.summary;
		totalCount = eventsPayload.total_count ?? eventsPayload.summary?.total_count ?? eventsPayload.items.length;
		scopeCounts = eventsPayload.scope_counts ?? scopeCountsForItems(eventsPayload.items);
		nextOffset = nextPageOffset(eventsPayload.links);
	} catch (error) {
		eventsBackendError = error instanceof Error ? error.message : 'Unknown backend error';
	}

	return {
		events,
		summary,
		totalCount,
		scopeCounts,
		nextOffset,
		pageSize: PAGE_SIZE,
		eventsBackendError,
		initialFilters: {
			protocol: protocol || null,
			target: target || null,
			scope: scope || null,
			search: searchText || null,
			severity: severity || null,
			coverage: coverage || null,
			tag: tag || null
		}
	};
}) satisfies PageServerLoad;
