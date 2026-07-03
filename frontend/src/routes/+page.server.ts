import { backendRequest, readBackendError } from '$lib/server/backend';
import { nextPageOffset } from '$lib/api-pagination';
import { canonicalProtocolSlug } from '$lib/catalog';
import { compactLongTokens } from '$lib/event-preview';
import type {
	ChainEvent,
	DefiProtocolsResponse,
	EventsResponse,
	ProgramVersionSummariesResponse
} from '$lib/types';
import type { PageServerLoad } from './$types';

const HOME_FETCH_TIMEOUT_MS = 2500;
const RECENT_DEFI_ALERT_LIMIT = 250;
const PROTOCOL_ALERT_LIMIT = 64;
const DAY_SECONDS = 86_400;
const FALLBACK_WINDOW_PAGE_SIZE = 500;
const FALLBACK_WINDOW_MAX_PAGES = 24;
const FALLBACK_WINDOW_BATCH_SIZE = 4;

type AlertSeverity = 'critical' | 'high' | 'standard';
type ActivityWindowFilter = {
	severity?: AlertSeverity;
	includeRelatedMultisigs?: boolean;
};
type ActivityWindowStats = {
	days: number;
	current: number;
	previous: number;
	percentChange: number | null;
	filteredByBackend: boolean;
};

type ProtocolAlertAccumulator = {
	slug: string;
	name: string;
	recentAlertCount: number;
	criticalCount: number;
	highCount: number;
	standardCount: number;
	latestSlot: number;
	latestTimestamp: number | null;
	latestSummary: string;
	latestEventId: string;
};
type HomeHeroAlert = {
	protocolSlug: string;
	protocolName: string;
	severity: AlertSeverity;
	summary: string;
	eventId: string;
	slot: number;
	timestamp: number | null;
};

async function backendJson<T>(fetch: typeof globalThis.fetch, path: string) {
	const controller = new AbortController();
	const timeout = setTimeout(() => controller.abort(), HOME_FETCH_TIMEOUT_MS);
	try {
		const response = await backendRequest(fetch, path, { signal: controller.signal });
		if (!response.ok) throw new Error(await readBackendError(response));
		return (await response.json()) as T;
	} finally {
		clearTimeout(timeout);
	}
}

async function eventTotal(fetch: typeof globalThis.fetch, searchParams: Record<string, string>) {
	const search = new URLSearchParams({ 'page[size]': '1', 'page[offset]': '0', ...searchParams });
	const response = await backendJson<EventsResponse>(fetch, `/api/events?${search.toString()}`);
	return response.total_count ?? response.items.length;
}

function eventTimestamp(event: ChainEvent) {
	return event.block_time ?? event.indexed_at;
}

function payloadString(event: ChainEvent, key: string) {
	const value = event.payload[key];
	return typeof value === 'string' ? value : null;
}

function eventSeverity(event: ChainEvent): AlertSeverity {
	const payloadSeverity = payloadString(event, 'severity');
	if (
		payloadSeverity === 'critical' ||
		event.tags.includes('severity:critical') ||
		event.tags.includes('signal:unknown_instruction')
	) {
		return 'critical';
	}

	if (
		payloadSeverity === 'high' ||
		event.tags.includes('severity:high') ||
		event.tags.includes('impact:high')
	) {
		return 'high';
	}

	return 'standard';
}

function percentChange(current: number, previous: number) {
	if (previous === 0) return current === 0 ? 0 : null;
	return Math.round(((current - previous) / previous) * 100);
}

function windowFilterParams(filter: ActivityWindowFilter): Record<string, string> {
	return {
		...(filter.severity ? { severity: filter.severity } : {}),
		...(filter.includeRelatedMultisigs ? { include_related_multisigs: 'true' } : {})
	};
}

function eventMatchesWindowFilter(event: ChainEvent, filter: ActivityWindowFilter) {
	return !filter.severity || eventSeverity(event) === filter.severity;
}

async function backendSupportsTimeFilters(fetch: typeof globalThis.fetch, nowTs: number) {
	const futureCount = await eventTotal(fetch, { from_ts: String(nowTs + DAY_SECONDS) });
	return futureCount === 0;
}

async function backendWindowStats(
	fetch: typeof globalThis.fetch,
	nowTs: number,
	days: number,
	filter: ActivityWindowFilter = {}
): Promise<ActivityWindowStats> {
	const windowSeconds = days * DAY_SECONDS;
	const currentStart = nowTs - windowSeconds;
	const previousStart = currentStart - windowSeconds;
	const filterParams = windowFilterParams(filter);
	const [current, previous] = await Promise.all([
		eventTotal(fetch, { ...filterParams, from_ts: String(currentStart), before_ts: String(nowTs) }),
		eventTotal(fetch, {
			...filterParams,
			from_ts: String(previousStart),
			before_ts: String(currentStart)
		})
	]);

	return {
		days,
		current,
		previous,
		percentChange: percentChange(current, previous),
		filteredByBackend: true
	};
}

async function fallbackWindowStats(
	fetch: typeof globalThis.fetch,
	nowTs: number,
	days: number,
	filter: ActivityWindowFilter = {}
): Promise<ActivityWindowStats> {
	const windowSeconds = days * DAY_SECONDS;
	const currentStart = nowTs - windowSeconds;
	const previousStart = currentStart - windowSeconds;
	let current = 0;
	let previous = 0;
	let stop = false;

	for (
		let batchStart = 0;
		batchStart < FALLBACK_WINDOW_MAX_PAGES && !stop;
		batchStart += FALLBACK_WINDOW_BATCH_SIZE
	) {
		const pages = await Promise.allSettled(
			Array.from({ length: FALLBACK_WINDOW_BATCH_SIZE }, (_, index) => {
				const pageIndex = batchStart + index;
				const offset = pageIndex * FALLBACK_WINDOW_PAGE_SIZE;
				const search = new URLSearchParams({
					'page[size]': String(FALLBACK_WINDOW_PAGE_SIZE),
					'page[offset]': String(offset),
					...windowFilterParams(filter)
				});
				return backendJson<EventsResponse>(
					fetch,
					`/api/events?${search.toString()}`
				);
			})
		);

		for (const result of pages) {
			if (result.status !== 'fulfilled') continue;

			const payload = result.value;
			if (payload.items.length === 0) {
				stop = true;
				continue;
			}

			for (const event of payload.items) {
				if (!eventMatchesWindowFilter(event, filter)) continue;
				const timestamp = eventTimestamp(event);
				if (timestamp >= currentStart && timestamp < nowTs) {
					current += 1;
				} else if (timestamp >= previousStart && timestamp < currentStart) {
					previous += 1;
				}
			}

			const oldestTimestamp = Math.min(...payload.items.map(eventTimestamp));
			if (oldestTimestamp < previousStart || nextPageOffset(payload.links) === null) {
				stop = true;
			}
		}
	}

	return {
		days,
		current,
		previous,
		percentChange: percentChange(current, previous),
		filteredByBackend: false
	};
}

async function activityWindowStats(
	fetch: typeof globalThis.fetch,
	filter: ActivityWindowFilter = {}
): Promise<ActivityWindowStats> {
	const nowTs = Math.floor(Date.now() / 1000);
	let useBackendFilters = false;

	try {
		useBackendFilters = await backendSupportsTimeFilters(fetch, nowTs);
	} catch {
		useBackendFilters = false;
	}

	const loadWindow = useBackendFilters
		? (days: number) => backendWindowStats(fetch, nowTs, days, filter)
		: (days: number) => fallbackWindowStats(fetch, nowTs, days, filter);
	return loadWindow(7);
}

function linkedProtocolSlugs(event: ChainEvent) {
	const linked = event.payload.linked_protocols;
	if (!Array.isArray(linked)) return [];

	return linked
		.map((item) => {
			if (!item || typeof item !== 'object') return null;
			const protocol = (item as Record<string, unknown>).protocol;
			return typeof protocol === 'string' ? protocol : null;
		})
		.filter((value): value is string => Boolean(value));
}

function eventProtocolSlugs(event: ChainEvent, knownProtocolSlugs: Set<string>) {
	const slugs = new Set<string>();
	for (const tag of event.tags) {
		if (!tag.startsWith('protocol:')) continue;
		const slug = canonicalProtocolSlug(tag.slice('protocol:'.length));
		if (knownProtocolSlugs.has(slug)) slugs.add(slug);
	}

	for (const slug of [
		payloadString(event, 'protocol'),
		payloadString(event, 'decoded_protocol'),
		...linkedProtocolSlugs(event)
	]) {
		const canonicalSlug = slug ? canonicalProtocolSlug(slug) : null;
		if (canonicalSlug && knownProtocolSlugs.has(canonicalSlug)) slugs.add(canonicalSlug);
	}

	return [...slugs];
}

function eventMultisigAddress(event: ChainEvent) {
	const multisig = payloadString(event, 'multisig');
	if (multisig) return multisig;
	return event.target_type === 'multisig' ? event.target_address : null;
}

function distinctMultisigCount(events: ChainEvent[]) {
	return new Set(events.map(eventMultisigAddress).filter((value): value is string => Boolean(value))).size;
}

function protocolAlertRows(
	events: ChainEvent[],
	protocolNames: Map<string, string>,
	knownProtocolSlugs: Set<string>
) {
	const rows = new Map<string, ProtocolAlertAccumulator>();

	for (const event of events) {
		const severity = eventSeverity(event);
		for (const slug of eventProtocolSlugs(event, knownProtocolSlugs)) {
			const row =
				rows.get(slug) ??
				({
					slug,
					name: protocolNames.get(slug) ?? slug.replaceAll('_', ' '),
					recentAlertCount: 0,
					criticalCount: 0,
					highCount: 0,
					standardCount: 0,
					latestSlot: 0,
					latestTimestamp: null,
					latestSummary: '',
					latestEventId: ''
				} satisfies ProtocolAlertAccumulator);

			row.recentAlertCount += 1;
			if (severity === 'critical') row.criticalCount += 1;
			if (severity === 'high') row.highCount += 1;
			if (severity === 'standard') row.standardCount += 1;
			if (event.slot > row.latestSlot) {
				row.latestSlot = event.slot;
				row.latestTimestamp = eventTimestamp(event);
				row.latestSummary = compactLongTokens(event.summary);
				row.latestEventId = event.id;
			}

			rows.set(slug, row);
		}
	}

	return [...rows.values()]
		.sort(
			(left, right) =>
				right.recentAlertCount - left.recentAlertCount ||
				right.criticalCount - left.criticalCount ||
				right.highCount - left.highCount ||
				right.latestSlot - left.latestSlot ||
				left.name.localeCompare(right.name)
		)
		.slice(0, PROTOCOL_ALERT_LIMIT);
}

function alertSeverityRank(severity: AlertSeverity) {
	if (severity === 'critical') return 3;
	if (severity === 'high') return 2;
	return 1;
}

function isBetterHeroAlert(candidate: HomeHeroAlert, current: HomeHeroAlert | null) {
	if (!current) return true;

	const rankDelta = alertSeverityRank(candidate.severity) - alertSeverityRank(current.severity);
	if (rankDelta !== 0) return rankDelta > 0;

	const candidateTimestamp = candidate.timestamp ?? 0;
	const currentTimestamp = current.timestamp ?? 0;
	return candidateTimestamp !== currentTimestamp
		? candidateTimestamp > currentTimestamp
		: candidate.slot > current.slot;
}

function heroAlertFromEvents(
	events: ChainEvent[],
	protocolNames: Map<string, string>,
	knownProtocolSlugs: Set<string>
) {
	let selected: HomeHeroAlert | null = null;

	for (const event of events) {
		const slugs = eventProtocolSlugs(event, knownProtocolSlugs);
		if (slugs.length === 0) continue;

		const severity = eventSeverity(event);
		for (const slug of slugs) {
			const candidate: HomeHeroAlert = {
				protocolSlug: slug,
				protocolName: protocolNames.get(slug) ?? slug.replaceAll('_', ' '),
				severity,
				summary: compactLongTokens(event.summary),
				eventId: event.id,
				slot: event.slot,
				timestamp: eventTimestamp(event)
			};

			if (isBetterHeroAlert(candidate, selected)) selected = candidate;
		}
	}

	return selected;
}

export const load = (async ({ fetch, parent }) => {
	const app = await parent();
	const protocolNames = new Map(app.defiProtocols.map((protocol) => [protocol.slug, protocol.name]));
	const knownProtocolSlugs = new Set(protocolNames.keys());
	const nowTs = Math.floor(Date.now() / 1000);
	const sevenDaysAgoTs = nowTs - 7 * DAY_SECONDS;

	const [
		recentDefiResult,
		protocolActivityResult,
		highResult,
		standardResult,
		versionResult,
		activityWindowResult,
		criticalActivityWindowResult
	] =
		await Promise.allSettled([
			backendJson<EventsResponse>(
				fetch,
				`/api/events?page%5Bsize%5D=${RECENT_DEFI_ALERT_LIMIT}&scope=defi&include_related_multisigs=true&from_ts=${sevenDaysAgoTs}&before_ts=${nowTs}`
			),
			backendJson<DefiProtocolsResponse>(fetch, '/api/protocols?include=activitySummary'),
			eventTotal(fetch, { severity: 'high' }),
			eventTotal(fetch, { severity: 'standard' }),
			backendJson<ProgramVersionSummariesResponse>(fetch, '/api/programs/version-summaries'),
			activityWindowStats(fetch, { includeRelatedMultisigs: true }),
			activityWindowStats(fetch, { severity: 'critical', includeRelatedMultisigs: true })
		]);

	const recentDefi = recentDefiResult.status === 'fulfilled' ? recentDefiResult.value : null;
	const recentDefiEvents = recentDefi?.items ?? [];
	const protocolActivityPayload =
		protocolActivityResult.status === 'fulfilled' ? protocolActivityResult.value : null;
	const homeProtocolTotals = Object.fromEntries(
		[
			...(protocolActivityPayload?.included?.activity_summaries ?? []).reduce((totals, summary) => {
				const slug = canonicalProtocolSlug(summary.slug);
				totals.set(slug, (totals.get(slug) ?? 0) + summary.total);
				return totals;
			}, new Map<string, number>())
		]
	);
	const versionSummaries = versionResult.status === 'fulfilled' ? versionResult.value.items : [];

	return {
		homeAlertCounts: {
			critical: recentDefi?.scope_counts.critical ?? 0,
			high: highResult.status === 'fulfilled' ? highResult.value : 0,
			standard: standardResult.status === 'fulfilled' ? standardResult.value : 0,
			defi: recentDefi?.total_count ?? 0,
			programUpdates: recentDefi?.scope_counts.programs ?? 0
		},
		homeProgramVersionStats: {
			withVersions: versionSummaries.filter((summary) => summary.version_count > 0).length,
			versions: versionSummaries.reduce((count, summary) => count + summary.version_count, 0),
			diffable: versionSummaries.reduce((count, summary) => count + summary.diffable_count, 0)
		},
		homeActivityWindow:
			activityWindowResult.status === 'fulfilled'
				? activityWindowResult.value
				: {
						days: 7,
						current: 0,
						previous: 0,
						percentChange: 0,
						filteredByBackend: false
					},
		homeCriticalActivityWindow:
			criticalActivityWindowResult.status === 'fulfilled'
				? criticalActivityWindowResult.value
				: {
						days: 7,
						current: 0,
						previous: 0,
						percentChange: 0,
						filteredByBackend: false
		},
		homeProtocolTotals,
		homeRelatedMultisigCount: distinctMultisigCount(recentDefiEvents),
		homeHeroAlert: heroAlertFromEvents(recentDefiEvents, protocolNames, knownProtocolSlugs),
		homeProtocolAlertRows: protocolAlertRows(recentDefiEvents, protocolNames, knownProtocolSlugs)
	};
}) satisfies PageServerLoad;
