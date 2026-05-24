<script lang="ts">
	import { browser } from '$app/environment';
	import { protocolDisplayName, protocolPageHref } from '$lib/catalog';
	import ActivityEventRow from '$lib/components/ActivityEventRow.svelte';
	import type { ChainEvent, EventsResponse } from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	type ActivityTab = 'all' | 'critical' | 'defi' | 'programs' | 'idl' | 'multisig';
	type SeverityFilter = 'all' | 'critical' | 'high' | 'standard';
	type CoverageFilter = 'all' | 'known';
	type EventQueryState = {
		protocol: string | null;
		target: string | null;
		scope: ActivityTab;
		search: string;
		severity: SeverityFilter;
		coverage: CoverageFilter;
		tag: string;
	};

	function scopeToTab(scope: string | null): ActivityTab {
		return scope === 'critical' ||
			scope === 'defi' ||
			scope === 'programs' ||
			scope === 'idl' ||
			scope === 'multisig'
			? scope
			: 'all';
	}

	function normalizeSeverityFilter(severity: string | null): SeverityFilter {
		return severity === 'critical' || severity === 'high' || severity === 'standard' ? severity : 'all';
	}

	function normalizeCoverageFilter(coverage: string | null): CoverageFilter {
		return coverage === 'known' ? 'known' : 'all';
	}

	let filtersOpen = $state(false);
	let activeTab = $derived.by(() => scopeToTab(data.initialFilters.scope));
	let severityFilter = $state<SeverityFilter>('all');
	let protocolFilter = $state('all');
	let coverageFilter = $state<CoverageFilter>('all');
	let tagFilter = $state('');
	let searchText = $state('');
	const initialChunk = (() => ({
		events: [...data.events],
		summary: data.summary,
		totalCount: data.totalCount,
		scopeCounts: data.scopeCounts,
		nextOffset: data.nextOffset
	}))();
	let events = $state<ChainEvent[]>(initialChunk.events);
	let summary = $state<PageData['summary']>(initialChunk.summary);
	let totalCount = $state(initialChunk.totalCount);
	let scopeCounts = $state<EventsResponse['scope_counts']>(initialChunk.scopeCounts);
	let nextOffset = $state<number | null>(initialChunk.nextOffset);
	let isLoadingMore = $state(false);
	let isRefreshing = $state(false);
	let loadMoreError = $state<string | null>(null);
	let refreshError = $state<string | null>(null);
	let loadSentinel = $state<HTMLDivElement | null>(null);
	let lastQuerySignature = $state('');
	let filterRequestId = 0;
	const emptyScopeCounts: EventsResponse['scope_counts'] = {
		all: 0,
		critical: 0,
		defi: 0,
		programs: 0,
		idl: 0,
		multisig: 0
	};

	$effect(() => {
		const initialQuery = queryStateFromData();
		severityFilter = initialQuery.severity;
		protocolFilter = initialQuery.protocol ?? 'all';
		coverageFilter = initialQuery.coverage;
		tagFilter = initialQuery.tag;
		searchText = initialQuery.search;
		events = data.events;
		summary = data.summary;
		totalCount = data.totalCount;
		scopeCounts = data.scopeCounts;
		nextOffset = data.nextOffset;
		loadMoreError = null;
		refreshError = null;
		lastQuerySignature = querySignature(initialQuery, 0);
	});

	$effect(() => {
		const queryState = currentQueryState();
		const signature = querySignature(queryState, 0);
		if (!browser || signature === lastQuerySignature) return;

		const timeout = window.setTimeout(
			() => {
				void refreshEvents(signature, queryState);
			},
			searchText.trim() ? 180 : 0
		);

		return () => window.clearTimeout(timeout);
	});

	const tabs = [
		{ id: 'all', label: 'All' },
		{ id: 'critical', label: 'Critical' },
		{ id: 'defi', label: 'DeFi' },
		{ id: 'programs', label: 'Programs' },
		{ id: 'idl', label: 'IDL' },
		{ id: 'multisig', label: 'Multisig' }
	] as const;

	function allEvents() {
		return events;
	}

	function queryStateFromData(): EventQueryState {
		return {
			protocol: data.initialFilters.protocol,
			target: data.initialFilters.target,
			scope: scopeToTab(data.initialFilters.scope),
			search: data.initialFilters.search ?? '',
			severity: normalizeSeverityFilter(data.initialFilters.severity),
			coverage: normalizeCoverageFilter(data.initialFilters.coverage),
			tag: data.initialFilters.tag ?? ''
		};
	}

	function currentQueryState(): EventQueryState {
		return {
			protocol: protocolFilter === 'all' ? null : protocolFilter,
			target: data.initialFilters.target,
			scope: activeTab,
			search: searchText.trim(),
			severity: severityFilter,
			coverage: coverageFilter,
			tag: tagFilter.trim()
		};
	}

	function buildSearchParams(offset: number, queryState: EventQueryState) {
		const search = new URLSearchParams({
			limit: String(data.pageSize),
			offset: String(offset)
		});

		if (queryState.protocol) {
			search.set('protocol', queryState.protocol);
			search.set('include_related_multisigs', 'true');
		}
		if (queryState.target) search.set('target_address', queryState.target);
		if (queryState.scope !== 'all') search.set('scope', queryState.scope);
		if (queryState.search) search.set('search', queryState.search);
		if (queryState.severity !== 'all') search.set('severity', queryState.severity);
		if (queryState.coverage !== 'all') search.set('coverage', queryState.coverage);
		if (queryState.tag) search.set('tag', queryState.tag);

		return search;
	}

	function querySignature(queryState: EventQueryState, offset: number) {
		return buildSearchParams(offset, queryState).toString();
	}

	function buildEventsUrl(offset: number, queryState = currentQueryState()) {
		return `${data.backendBaseUrl}/api/events?${buildSearchParams(offset, queryState).toString()}`;
	}

	async function fetchEventsPage(offset: number, queryState: EventQueryState) {
		const response = await fetch(buildEventsUrl(offset, queryState));
		if (!response.ok) {
			throw new Error(`Backend returned ${response.status}`);
		}

		return (await response.json()) as EventsResponse;
	}

	function responseTotalCount(payload: EventsResponse) {
		return payload.total_count ?? payload.summary?.total_count ?? payload.items.length;
	}

	function responseScopeCounts(payload: EventsResponse) {
		return payload.scope_counts ?? scopeCountsForItems(payload.items);
	}

	async function refreshEvents(expectedSignature: string, queryState: EventQueryState) {
		const requestId = ++filterRequestId;
		isRefreshing = true;
		loadMoreError = null;
		refreshError = null;

		try {
			const payload = await fetchEventsPage(0, queryState);
			if (requestId !== filterRequestId || querySignature(currentQueryState(), 0) !== expectedSignature) return;

			events = payload.items;
			summary = payload.summary;
			totalCount = responseTotalCount(payload);
			scopeCounts = responseScopeCounts(payload);
			nextOffset = payload.next_offset;
			lastQuerySignature = expectedSignature;
		} catch (error) {
			if (requestId === filterRequestId) {
				refreshError = error instanceof Error ? error.message : 'Could not refresh events';
			}
		} finally {
			if (requestId === filterRequestId) {
				isRefreshing = false;
			}
		}
	}

	async function loadMoreEvents() {
		if (!browser || isLoadingMore || isRefreshing || nextOffset === null) return;

		const requestedOffset = nextOffset;
		const queryState = currentQueryState();
		const requestSignature = querySignature(queryState, requestedOffset);
		const requestId = filterRequestId;
		isLoadingMore = true;
		loadMoreError = null;

		try {
			const payload = await fetchEventsPage(requestedOffset, queryState);
			if (requestId !== filterRequestId || querySignature(currentQueryState(), requestedOffset) !== requestSignature) return;

			const seen = new Set(events.map((event) => event.id));
			const appended = payload.items.filter((event) => !seen.has(event.id));
			events = [...events, ...appended];
			summary = payload.summary;
			totalCount = responseTotalCount(payload);
			scopeCounts = responseScopeCounts(payload);
			nextOffset = payload.next_offset;
		} catch (error) {
			loadMoreError = error instanceof Error ? error.message : 'Could not load more events';
		} finally {
			isLoadingMore = false;
		}
	}

	$effect(() => {
		if (!browser || !loadSentinel) return;

		const observer = new IntersectionObserver(
			(entries) => {
				if (entries.some((entry) => entry.isIntersecting)) {
					void loadMoreEvents();
				}
			},
			{ rootMargin: '720px 0px' }
		);

		observer.observe(loadSentinel);

		return () => observer.disconnect();
	});

	function payloadString(event: ChainEvent, key: string) {
		const value = event.payload[key];
		return typeof value === 'string' && value.length > 0 ? value : null;
	}

	function isIdlEvent(event: ChainEvent) {
		return event.kind.startsWith('program_idl_');
	}

	function isProgramUpdateEvent(event: ChainEvent) {
		return (
			event.kind === 'program_deployed' ||
			event.kind === 'program_upgraded' ||
			event.kind === 'program_finalized' ||
			event.kind === 'program_migrated_to_loader_v4' ||
			isIdlEvent(event)
		);
	}

	function isCriticalEvent(event: ChainEvent) {
		return event.tags.includes('severity:critical') || event.tags.includes('signal:unknown_instruction');
	}

	function severity(event: ChainEvent) {
		if (isCriticalEvent(event)) return 'critical';
		if (event.tags.includes('severity:high') || event.tags.includes('impact:high')) return 'high';
		return 'standard';
	}

	function protocolTag(event: ChainEvent) {
		return event.tags.find((tag) => tag.startsWith('protocol:')) ?? null;
	}

	function currentProtocolLabel() {
		if (protocolFilter === 'all') return null;
		return protocolDisplayName(data.defiProtocols, protocolFilter);
	}

	function scopedHref(scope: typeof activeTab) {
		const search = new URLSearchParams();
		if (protocolFilter !== 'all') search.set('protocol', protocolFilter);
		if (data.initialFilters.target) search.set('target', data.initialFilters.target);
		if (scope !== 'all') search.set('scope', scope);
		if (searchText.trim()) search.set('search', searchText.trim());
		if (severityFilter !== 'all') search.set('severity', severityFilter);
		if (coverageFilter !== 'all') search.set('coverage', coverageFilter);
		if (tagFilter.trim()) search.set('tag', tagFilter.trim());
		const query = search.toString();
		return query ? `/events?${query}` : '/events';
	}

	function protocolOptions() {
		const options = data.defiProtocols.map((protocol) => ({
			slug: protocol.slug,
			name: protocol.name
		}));

		if (!options.some((option) => option.slug === 'squads')) {
			options.push({ slug: 'squads', name: 'Squads' });
		}

		return options;
	}

	function activeFilterBadges() {
		const badges: string[] = [];

		if (severityFilter !== 'all') badges.push(`Severity: ${severityFilter}`);
		if (protocolFilter !== 'all') badges.push(`Protocol: ${protocolDisplayName(data.defiProtocols, protocolFilter)}`);
		if (coverageFilter === 'known') badges.push('Known DeFi only');
		if (tagFilter.trim()) badges.push(`Tag: ${tagFilter.trim()}`);
		if (searchText.trim()) badges.push(`Search: ${searchText.trim()}`);

		return badges;
	}

	function activeFilterCount() {
		return activeFilterBadges().length;
	}

	function matchesTab(event: ChainEvent) {
		if (activeTab === 'all') return true;
		if (activeTab === 'critical') return isCriticalEvent(event);
		if (activeTab === 'defi') return event.tags.includes('watch:defi');
		if (activeTab === 'programs') return isProgramUpdateEvent(event) && !isIdlEvent(event);
		if (activeTab === 'idl') return isIdlEvent(event);
		if (activeTab === 'multisig') return event.target_type === 'multisig';
		return true;
	}

	function matchesFilters(event: ChainEvent) {
		if (!matchesTab(event)) return false;
		if (severityFilter !== 'all' && severity(event) !== severityFilter) return false;
		if (coverageFilter === 'known' && !event.tags.includes('watch:defi')) return false;
		if (protocolFilter !== 'all' && protocolTag(event) !== `protocol:${protocolFilter}`) return false;

		if (tagFilter.trim()) {
			const needle = tagFilter.trim().toLowerCase();
			if (!event.tags.some((tag) => tag.toLowerCase().includes(needle))) return false;
		}

		if (searchText.trim()) {
			const needle = searchText.trim().toLowerCase();
			const haystack = [
				event.summary,
				event.kind,
				event.target_address,
				event.actor ?? '',
				payloadString(event, 'protocol_name') ?? '',
				payloadString(event, 'instruction_name') ?? '',
				...event.tags
			]
				.join(' ')
				.toLowerCase();
			if (!haystack.includes(needle)) return false;
		}

		return true;
	}

	function filteredEvents() {
		return allEvents().filter(matchesFilters);
	}

	function tabCount(id: (typeof tabs)[number]['id']) {
		return scopeCounts[id] ?? 0;
	}

	function scopeCountsForItems(items: ChainEvent[]): EventsResponse['scope_counts'] {
		return items.reduce(
			(counts, event) => {
				counts.all += 1;
				if (isCriticalEvent(event)) counts.critical += 1;
				if (event.tags.includes('watch:defi')) counts.defi += 1;
				if (isProgramUpdateEvent(event) && !isIdlEvent(event)) counts.programs += 1;
				if (isIdlEvent(event)) counts.idl += 1;
				if (event.target_type === 'multisig') counts.multisig += 1;
				return counts;
			},
			{ ...emptyScopeCounts }
		);
	}

	function resetFilters() {
		severityFilter = 'all';
		protocolFilter = 'all';
		coverageFilter = 'all';
		tagFilter = '';
		searchText = '';
	}

</script>

<svelte:head>
	<title>FireWatch Activity</title>
</svelte:head>

<div class="page-shell">
	<section class="panel activity-header">
		<div class="header-row">
			<h1>Activity</h1>
			<div class="header-stats">
			<span>Loaded {allEvents().length}</span>
			<span>Visible {filteredEvents().length}</span>
			<span>Critical {tabCount('critical')}</span>
			<span>Matching {totalCount}</span>
			</div>
		</div>
		{#if currentProtocolLabel()}
			<p class="context-line">
				Filtered for {currentProtocolLabel()}.
				<a href={protocolPageHref(protocolFilter)}>Open protocol page</a>
			</p>
		{/if}
		{#if data.eventsBackendError}
			<p class="flash error">Events unavailable: {data.eventsBackendError}</p>
		{/if}
		{#if refreshError}
			<p class="flash error">Could not refresh events: {refreshError}</p>
		{/if}
	</section>

		<section class="panel sticky-panel">
			<div class="toolbar-top">
				<div class="tab-row" role="tablist" aria-label="Activity views">
				{#each tabs as tab}
					<a class:active={activeTab === tab.id} class="tab-button" href={scopedHref(tab.id)}>
						{tab.label}
						<span>{tabCount(tab.id)}</span>
					</a>
				{/each}
			</div>

				<div class="toolbar-actions">
					<input
						class="quick-search"
						bind:value={searchText}
						type="search"
						placeholder="Search activity"
						aria-label="Search activity"
					/>
					{#if isRefreshing}
						<span class="toolbar-meta">Updating</span>
					{/if}
					{#if activeFilterCount() > 0}
						<span class="toolbar-meta">{activeFilterCount()} filters</span>
					{/if}
					<button class="filter-toggle" type="button" onclick={() => (filtersOpen = !filtersOpen)}>
						{filtersOpen ? 'Hide filters' : 'Filters'}
				</button>
			</div>
		</div>

			{#if activeFilterBadges().length > 0}
				<div class="active-filter-row">
					{#each activeFilterBadges() as badge}
						<span class="active-filter">{badge}</span>
					{/each}
					<a class="shortcut-button reset-button" href="/events" onclick={resetFilters}>Reset</a>
				</div>
		{/if}

		{#if filtersOpen}
			<div class="filter-drawer">
				<div class="filter-grid">
					<label>
						<span>Severity</span>
						<select bind:value={severityFilter}>
							<option value="all">All</option>
							<option value="critical">Critical</option>
							<option value="high">High</option>
							<option value="standard">Standard</option>
						</select>
					</label>

					<label>
						<span>Protocol</span>
						<select bind:value={protocolFilter}>
							<option value="all">All protocols</option>
							{#each protocolOptions() as protocol}
								<option value={protocol.slug}>{protocol.name}</option>
							{/each}
						</select>
					</label>

					<label>
						<span>Catalog</span>
						<select bind:value={coverageFilter}>
							<option value="all">All programs</option>
							<option value="known">FireWatch-known DeFi only</option>
						</select>
					</label>

					<label>
						<span>Tag</span>
						<input bind:value={tagFilter} type="text" placeholder="signal:idl_update" />
					</label>

				</div>

					<div class="shortcut-row">
						<a class="shortcut-button" href={scopedHref('critical')}>Critical only</a>
					<a class="shortcut-button" href={scopedHref('idl')}>IDL changes</a>
					<a class="shortcut-button" href={scopedHref('programs')}>Program updates</a>
						<button class="shortcut-button" type="button" onclick={() => (tagFilter = 'signal:unknown_instruction')}>
							Unknown instructions
						</button>
					</div>
				</div>
			{/if}
		</section>

	<section class="events-grid">
		{#if filteredEvents().length === 0}
			<div class="panel empty-panel">
				<p class="empty-state">No events match the current filters.</p>
			</div>
		{:else}
			{#each filteredEvents() as event}
				<ActivityEventRow
					{event}
					explorerCluster={data.explorerCluster}
					protocols={data.defiProtocols}
					activeTag={tagFilter}
					onTagSelect={(tag) => (tagFilter = tag)}
				/>
			{/each}
		{/if}
	</section>

	<div bind:this={loadSentinel} class="scroll-sentinel" aria-hidden="true"></div>

	{#if isLoadingMore || loadMoreError || nextOffset !== null}
		<div class="load-row pagination-row">
			{#if loadMoreError}
				<span class="pagination-state error-state">Could not load more events.</span>
				<button class="load-button" type="button" onclick={loadMoreEvents}>Try again</button>
			{:else if isLoadingMore}
				<span class="pagination-state">Loading more…</span>
			{:else if nextOffset !== null}
				<span class="pagination-state">More events load automatically as you scroll.</span>
			{/if}
		</div>
	{/if}
</div>

<style>
	.page-shell {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem 0 2.4rem;
		display: grid;
		gap: 0.86rem;
	}

	.panel {
		background: #fff;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 10px;
	}

	.panel {
		padding: 0.86rem 0.96rem;
	}

	.activity-header {
		display: grid;
		gap: 0.4rem;
	}

	.header-row,
	.header-stats,
	.tab-row,
	.toolbar-actions,
	.active-filter-row,
	.filter-grid,
	.shortcut-row,
	.load-row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.46rem;
	}

	.scroll-sentinel {
		height: 1px;
		width: 100%;
	}

	h1,
	p {
		margin: 0;
	}

	.empty-state,
	.context-line,
	.header-stats span {
		color: #5f6c79;
	}

	h1 {
		font-size: 1.18rem;
		line-height: 1.2;
		letter-spacing: 0;
	}

	.header-row {
		align-items: center;
		justify-content: space-between;
		gap: 0.7rem;
	}

	.header-stats {
		gap: 0;
	}

	.header-stats span {
		display: inline-flex;
		align-items: baseline;
		padding-right: 0.7rem;
		margin-right: 0.7rem;
		border-right: 1px solid rgba(23, 32, 42, 0.1);
		font-size: 0.78rem;
		font-weight: 600;
	}

	.header-stats span:last-child {
		padding-right: 0;
		margin-right: 0;
		border-right: none;
	}

	.context-line {
		font-size: 0.84rem;
	}

	.context-line a {
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.error-state {
		color: #b54835;
	}

	.sticky-panel {
		position: sticky;
		top: 4.45rem;
		z-index: 20;
		background: #fff;
		display: grid;
		gap: 0.55rem;
		box-shadow: 0 2px 8px rgba(23, 32, 42, 0.05);
	}

	.toolbar-top {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
	}

	.toolbar-actions {
		align-items: center;
		justify-content: flex-end;
	}

	.quick-search {
		width: min(17rem, 42vw);
		min-width: 11rem;
		padding: 0.46rem 0.62rem;
	}

	.toolbar-meta {
		font-size: 0.78rem;
		font-weight: 600;
		color: #5f6c79;
	}

	.filter-drawer {
		display: grid;
		gap: 0.6rem;
		padding-top: 0.08rem;
		border-top: 1px solid rgba(23, 32, 42, 0.06);
	}

	.filter-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
		gap: 0.6rem;
	}

	.tab-button,
	.shortcut-button,
	.filter-toggle {
		font: inherit;
	}

	.tab-button {
		padding: 0.42rem 0;
		border: none;
		border-bottom: 2px solid transparent;
		background: transparent;
		color: #556473;
		cursor: pointer;
		font-weight: 600;
		font-size: 0.82rem;
		text-decoration: none;
	}

	.shortcut-button,
	.filter-toggle {
		padding: 0.42rem 0.62rem;
		border: 1px solid rgba(23, 32, 42, 0.09);
		background: #fff;
		color: #556473;
		cursor: pointer;
		font-weight: 600;
		font-size: 0.8rem;
		border-radius: 8px;
		text-decoration: none;
	}

	.tab-button.active {
		color: #bd4a17;
		border-bottom-color: #bd4a17;
	}

	.filter-toggle {
		color: #17202a;
	}

	.tab-button span {
		margin-left: 0.36rem;
		font-size: 0.76rem;
		color: #7a8896;
	}

	.active-filter-row {
		align-items: center;
		gap: 0.5rem;
	}

	.active-filter {
		font-size: 0.78rem;
		color: #5f6c79;
	}

	.reset-button {
		background: transparent;
	}

	label {
		display: grid;
		gap: 0.28rem;
	}

	label span {
		font-size: 0.76rem;
		font-weight: 600;
		color: #63717d;
	}

	input,
	select {
		width: 100%;
		min-width: 0;
		box-sizing: border-box;
		padding: 0.54rem 0.68rem;
		border-radius: 8px;
		border: 1px solid rgba(23, 32, 42, 0.12);
		background: #fff;
		font: inherit;
		font-size: 0.88rem;
	}

	.events-grid {
		display: grid;
		gap: 0.52rem;
	}

	.load-row {
		align-items: center;
		gap: 0.8rem;
	}

	.load-button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 0.62rem 0.82rem;
		border: 1px solid rgba(23, 32, 42, 0.12);
		border-radius: 8px;
		background: #fff;
		color: #17202a;
		font: inherit;
		font-weight: 600;
		cursor: pointer;
		text-decoration: none;
	}

	.pagination-row {
		justify-content: space-between;
	}

	.pagination-state {
		font-size: 0.82rem;
		font-weight: 700;
		color: #6d7884;
	}

	.flash {
		margin: 0;
		padding: 0.72rem 0.86rem;
		border-radius: 8px;
		font-size: 0.88rem;
		font-weight: 600;
	}

	.flash.error {
		background: rgba(191, 74, 57, 0.14);
		color: #9c3929;
	}

	.empty-panel {
		padding: 1.2rem;
	}

	@media (max-width: 980px) {
		.toolbar-top {
			flex-direction: column;
			align-items: flex-start;
		}
	}

	@media (max-width: 720px) {
		.page-shell {
			width: min(100vw - 1.25rem, var(--fw-page-max));
			padding-top: 0.62rem;
			gap: 0.72rem;
		}

		.panel {
			border-radius: 8px;
			padding: 0.8rem;
		}

		.header-row {
			align-items: flex-start;
			flex-direction: column;
		}

		.header-stats {
			display: grid;
			grid-template-columns: repeat(2, minmax(0, 1fr));
			gap: 1px;
			width: 100%;
			overflow: hidden;
			border: 1px solid rgba(23, 32, 42, 0.07);
			border-radius: 8px;
			background: rgba(23, 32, 42, 0.07);
		}

		.header-stats span {
			display: grid;
			margin: 0;
			padding: 0.54rem 0.62rem;
			border-right: 0;
			background: #fbfaf8;
			line-height: 1.25;
		}

		.header-stats span:last-child {
			padding-right: 0.62rem;
		}

		.quick-search {
			width: 100%;
		}

		.sticky-panel {
			position: static;
		}

		.toolbar-actions {
			display: grid;
			grid-template-columns: minmax(0, 1fr) auto;
			align-items: center;
			width: 100%;
		}

		.toolbar-top {
			gap: 0.62rem;
		}

		.tab-row {
			flex-wrap: nowrap;
			width: 100%;
			overflow-x: auto;
			padding-bottom: 0.05rem;
			scrollbar-width: none;
		}

		.tab-row::-webkit-scrollbar,
		.shortcut-row::-webkit-scrollbar {
			display: none;
		}

		.tab-button {
			flex: 0 0 auto;
		}

		.quick-search {
			grid-column: 1 / -1;
		}

		.filter-toggle {
			margin-left: 0;
		}

		.filter-grid {
			grid-template-columns: 1fr;
		}

		.shortcut-row {
			flex-wrap: nowrap;
			overflow-x: auto;
			padding-bottom: 0.05rem;
			scrollbar-width: none;
		}

		.shortcut-button {
			flex: 0 0 auto;
			white-space: nowrap;
		}
	}
</style>
