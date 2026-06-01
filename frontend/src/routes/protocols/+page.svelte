<script lang="ts">
	import { page } from '$app/state';
	import { protocolActivityPageHref, protocolPageHref } from '$lib/catalog';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import { projectRiskSummary } from '$lib/risk';
	import type { ChainEvent, DefiProtocolDescriptor, EventsResponse, ProgramRiskSummary } from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	type ProtocolActivitySummary = {
		total: number;
		critical: number;
		high: number;
		standard: number;
		error: string | null;
	};

	type ProtocolCard = {
		protocol: DefiProtocolDescriptor;
		risk: ProgramRiskSummary;
		activity: ProtocolActivitySummary;
		searchText: string;
	};

	const EMPTY_ACTIVITY: ProtocolActivitySummary = {
		total: 0,
		critical: 0,
		high: 0,
		standard: 0,
		error: null
	};

	let searchQuery = $state(page.url.searchParams.get('search') ?? '');
	let remoteSearchMatches = $state<Record<string, string>>({});
	let remoteSearchLoading = $state(false);
	let remoteSearchError = $state<string | null>(null);
	let remoteSearchRun = 0;

	let rows = $derived.by(() =>
		data.defiProtocols
			.map((protocol) => {
				const activity = data.protocolActivitySummaries[protocol.slug] ?? EMPTY_ACTIVITY;
				return {
					protocol,
					risk: projectRiskSummary(protocol),
					activity,
					searchText: protocolSearchText(protocol)
				};
			})
			.sort(protocolSort)
	);
	let normalizedSearch = $derived(normalizeSearch(searchQuery));
	let knownProtocolSlugs = $derived(data.defiProtocols.map((protocol) => protocol.slug));
	let visibleRows = $derived.by(() =>
		rows.filter(
			(row) =>
				!normalizedSearch ||
				row.searchText.includes(normalizedSearch) ||
				Boolean(remoteSearchMatches[row.protocol.slug])
		)
	);
	let loadedActivityCount = $derived(
		rows.reduce((count, row) => count + (row.activity.error ? 0 : 1), 0)
	);

	$effect(() => {
		const query = searchQuery.trim();
		remoteSearchMatches = {};
		remoteSearchError = null;

		if (!isAddressSearch(query)) {
			remoteSearchLoading = false;
			return;
		}

		const run = ++remoteSearchRun;
		const controller = new AbortController();
		const debounce = window.setTimeout(async () => {
			remoteSearchLoading = true;

			try {
				const matches = await loadRemoteSearchMatches(query, controller.signal);
				if (run === remoteSearchRun) {
					remoteSearchMatches = matches;
				}
			} catch (error) {
				if (!controller.signal.aborted && run === remoteSearchRun) {
					remoteSearchError = error instanceof Error ? error.message : 'Activity search unavailable';
				}
			} finally {
				if (run === remoteSearchRun) {
					remoteSearchLoading = false;
				}
			}
		}, 220);

		return () => {
			controller.abort();
			window.clearTimeout(debounce);
		};
	});

	function normalizeSearch(value: string) {
		return value.trim().toLowerCase();
	}

	function protocolSearchText(protocol: DefiProtocolDescriptor) {
		return [
			protocol.name,
			protocol.slug,
			protocol.program_id,
			...protocol.programs.flatMap((program) => [
				program.label,
				program.program_id,
				program.idl_metadata_address,
				program.anchor_legacy_idl_address ?? ''
			]),
			...protocol.sol_domains
		]
			.filter(Boolean)
			.join(' ')
			.toLowerCase();
	}

	function protocolSort(left: ProtocolCard, right: ProtocolCard) {
		return (
			right.activity.total - left.activity.total ||
			right.activity.critical - left.activity.critical ||
			right.risk.unreviewed_critical - left.risk.unreviewed_critical ||
			right.risk.critical_total - left.risk.critical_total ||
			left.protocol.name.localeCompare(right.protocol.name)
		);
	}

	function formatCount(value: number | null | undefined) {
		return new Intl.NumberFormat('en-US').format(value ?? 0);
	}

	function formatActivityCount(activity: ProtocolActivitySummary, value: number) {
		return activity.error ? '-' : formatCount(value);
	}

	function plural(value: number, singular: string, pluralLabel = `${singular}s`) {
		return value === 1 ? singular : pluralLabel;
	}

	function ruleCount(protocol: DefiProtocolDescriptor) {
		const instructionRules = [...protocol.admin_instruction_filters];
		for (const instruction of protocol.high_impact_instruction_filters) {
			if (!instructionRules.includes(instruction)) instructionRules.push(instruction);
		}

		return (
			instructionRules.length + protocol.amount_triggers.length + (protocol.unknown_alerts ? 1 : 0)
		);
	}

	function knownSurfaceCount(protocol: DefiProtocolDescriptor) {
		return protocol.programs.length + protocol.sol_domains.length + ruleCount(protocol);
	}

	function criticalActivityHref(slug: string) {
		return `${protocolActivityPageHref(slug)}?severity=critical`;
	}

	function unresolvedHref(slug: string) {
		return protocolPageHref(slug);
	}

	function localMatchLabel(protocol: DefiProtocolDescriptor) {
		if (!normalizedSearch) return null;

		if (
			protocol.name.toLowerCase().includes(normalizedSearch) ||
			protocol.slug.toLowerCase().includes(normalizedSearch)
		) {
			return 'Protocol name';
		}

		const program = protocol.programs.find((candidate) =>
			[candidate.label, candidate.program_id, candidate.idl_metadata_address, candidate.anchor_legacy_idl_address]
				.filter(Boolean)
				.some((value) => value?.toLowerCase().includes(normalizedSearch))
		);
		if (program) {
			return `Program surface: ${program.label}`;
		}

		const domain = protocol.sol_domains.find((candidate) =>
			candidate.toLowerCase().includes(normalizedSearch)
		);
		if (domain) {
			return `.sol domain: ${domain}`;
		}

		return remoteSearchMatches[protocol.slug] ?? null;
	}

	function isAddressSearch(value: string) {
		return /^[1-9A-HJ-NP-Za-km-z]{20,64}$/.test(value.trim());
	}

	function payloadString(event: ChainEvent, key: string) {
		const value = event.payload[key];
		return typeof value === 'string' ? value : null;
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

	function eventProtocolSlugs(event: ChainEvent) {
		const slugs: string[] = [];

		function addKnownSlug(slug: string | null) {
			if (!slug || !knownProtocolSlugs.includes(slug) || slugs.includes(slug)) return;
			slugs.push(slug);
		}

		for (const tag of event.tags) {
			if (!tag.startsWith('protocol:')) continue;
			const slug = tag.slice('protocol:'.length);
			addKnownSlug(slug);
		}

		for (const slug of [
			payloadString(event, 'protocol'),
			payloadString(event, 'decoded_protocol'),
			payloadString(event, 'protocol_slug'),
			...linkedProtocolSlugs(event)
		]) {
			addKnownSlug(slug);
		}

		return slugs;
	}

	async function eventsPayload(search: URLSearchParams, signal: AbortSignal) {
		const response = await fetch(`${data.backendBaseUrl}/api/events?${search.toString()}`, {
			signal
		});
		if (!response.ok) throw new Error('Activity search unavailable');
		return (await response.json()) as EventsResponse;
	}

	async function loadRemoteSearchMatches(query: string, signal: AbortSignal) {
		const search = new URLSearchParams({ 'page[size]': '80', 'page[offset]': '0', search: query });
		const exactTarget = new URLSearchParams({
			'page[size]': '80',
			'page[offset]': '0',
			target_address: query
		});
		const payloads = await Promise.all([
			eventsPayload(search, signal),
			eventsPayload(exactTarget, signal)
		]);
		const matches: Record<string, string> = {};

		for (const event of payloads.flatMap((payload) => payload.items)) {
			for (const slug of eventProtocolSlugs(event)) {
				matches[slug] = event.target_address === query ? 'Activity target address' : 'Activity address';
			}
		}

		return matches;
	}
</script>

<svelte:head>
	<title>Tracked protocols · FireWatch</title>
</svelte:head>

<div class="page-shell">
	<section class="panel page-top">
		<div class="title-copy">
			<h1>Tracked protocols</h1>
			<p>Sorted by indexed activity. Search by protocol, program address, .sol domain, or activity address.</p>
		</div>

		<label class="search-box">
			<span>Search protocols</span>
			<input
				bind:value={searchQuery}
				type="search"
				placeholder="Name, program address, multisig wallet, .sol"
				autocomplete="off"
				spellcheck="false"
			/>
		</label>

		<div class="result-meta">
			<span>{formatCount(visibleRows.length)} of {formatCount(rows.length)} protocols</span>
			{#if loadedActivityCount > 0}
				<span>Sorted by activity</span>
			{:else}
				<span>Activity index unavailable in this preview</span>
			{/if}
			<span>{formatCount(loadedActivityCount)} activity indexes loaded</span>
			{#if remoteSearchLoading}
				<span>Checking indexed addresses...</span>
			{/if}
			{#if remoteSearchError}
				<span class="warning">{remoteSearchError}</span>
			{/if}
		</div>
	</section>

	<section class="protocol-grid" aria-label="Tracked protocol cards">
		{#if visibleRows.length === 0}
			<div class="panel empty-state">
				<p>No protocol matched this search yet.</p>
			</div>
		{:else}
			{#each visibleRows as row (row.protocol.slug)}
				<article class="protocol-card">
					<div class="card-header">
						<a class="protocol-name" href={protocolPageHref(row.protocol.slug)}>
							<ProtocolIdentity slug={row.protocol.slug} label={row.protocol.name} />
						</a>
						<a
							class={`grade grade-${row.risk.grade.toLowerCase()}`}
							href={protocolPageHref(row.protocol.slug)}
							aria-label={`Open ${row.protocol.name} score detail`}
						>
							Grade {row.risk.grade}
						</a>
					</div>

					{#if localMatchLabel(row.protocol)}
						<p class="match-line">Match: {localMatchLabel(row.protocol)}</p>
					{/if}

					<div class="activity-strip" aria-label={`${row.protocol.name} activity summary`}>
						<a href={protocolActivityPageHref(row.protocol.slug)}>
							<span>All activity</span>
							<strong>{formatActivityCount(row.activity, row.activity.total)}</strong>
						</a>
						<a href={criticalActivityHref(row.protocol.slug)}>
							<span>Critical</span>
							<strong>{formatActivityCount(row.activity, row.activity.critical)}</strong>
						</a>
						<a href={unresolvedHref(row.protocol.slug)}>
							<span>Unresolved</span>
							<strong>{formatCount(row.risk.unreviewed_critical)}</strong>
						</a>
					</div>

					<div class="surface-row" aria-label={`${row.protocol.name} tracked surfaces`}>
						<span>
							<strong>{formatCount(row.protocol.programs.length)}</strong>
							{plural(row.protocol.programs.length, 'program')}
						</span>
						<span>
							<strong>{formatCount(row.protocol.sol_domains.length)}</strong>
							.sol
						</span>
						<span>
							<strong>{formatCount(ruleCount(row.protocol))}</strong>
							{plural(ruleCount(row.protocol), 'rule')}
						</span>
						<span>
							<strong>{formatCount(knownSurfaceCount(row.protocol))}</strong>
							{plural(knownSurfaceCount(row.protocol), 'surface')}
						</span>
					</div>

					<div class="card-actions">
						<a href={protocolActivityPageHref(row.protocol.slug)}>Open activity</a>
						<a href={protocolPageHref(row.protocol.slug)}>Open protocol</a>
					</div>
				</article>
			{/each}
		{/if}
	</section>
</div>

<style>
	.page-shell {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem 0 2.4rem;
		display: grid;
		gap: 0.86rem;
	}

	.panel,
	.protocol-card {
		background: #fff;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 10px;
	}

	.panel {
		padding: 0.92rem 1rem;
	}

	h1,
	p {
		margin: 0;
	}

	h1 {
		font-size: 1.82rem;
		line-height: 1.04;
		letter-spacing: 0;
	}

	p,
	.empty-state {
		color: #5f6c79;
	}

	.page-top {
		display: grid;
		grid-template-columns: minmax(260px, 1fr) minmax(280px, 440px);
		gap: 0.8rem 1rem;
		align-items: end;
	}

	.title-copy {
		display: grid;
		gap: 0.25rem;
	}

	.search-box {
		display: grid;
		gap: 0.32rem;
		color: #596775;
		font-size: 0.78rem;
		font-weight: 750;
	}

	.search-box input {
		width: 100%;
		min-height: 2.45rem;
		border: 1px solid rgba(23, 32, 42, 0.14);
		border-radius: 8px;
		padding: 0.48rem 0.62rem;
		background: #fff;
		color: #17202a;
		font: inherit;
		font-size: 0.9rem;
		font-weight: 650;
		outline: none;
	}

	.search-box input:focus {
		border-color: rgba(189, 74, 23, 0.55);
		box-shadow: 0 0 0 3px rgba(189, 74, 23, 0.1);
	}

	.result-meta {
		grid-column: 1 / -1;
		display: flex;
		flex-wrap: wrap;
		gap: 0.32rem 0.7rem;
		color: #687684;
		font-size: 0.78rem;
		font-weight: 700;
	}

	.warning {
		color: #9c3929;
	}

	.protocol-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 0.78rem;
	}

	.empty-state {
		grid-column: 1 / -1;
		padding: 1rem;
	}

	.protocol-card {
		min-width: 0;
		padding: 0.84rem;
		display: grid;
		gap: 0.72rem;
	}

	.card-header {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 0.7rem;
		align-items: center;
	}

	.protocol-name {
		min-width: 0;
		text-decoration: none;
	}

	.grade {
		display: inline-flex;
		align-items: center;
		min-height: 1.75rem;
		padding: 0.24rem 0.48rem;
		border: 1px solid rgba(33, 109, 77, 0.22);
		border-radius: 8px;
		background: rgba(33, 109, 77, 0.08);
		color: #216d4d;
		font-size: 0.78rem;
		font-weight: 800;
		text-decoration: none;
		white-space: nowrap;
	}

	.grade-c,
	.grade-d {
		border-color: rgba(138, 94, 30, 0.22);
		background: rgba(138, 94, 30, 0.09);
		color: #8a5e1e;
	}

	.grade-f {
		border-color: rgba(156, 57, 41, 0.22);
		background: rgba(156, 57, 41, 0.09);
		color: #9c3929;
	}

	.match-line {
		color: #6a4f3c;
		font-size: 0.78rem;
		font-weight: 700;
	}

	.activity-strip {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		overflow: hidden;
	}

	.activity-strip a {
		min-width: 0;
		padding: 0.58rem 0.62rem;
		display: grid;
		gap: 0.14rem;
		text-decoration: none;
		border-right: 1px solid rgba(23, 32, 42, 0.08);
	}

	.activity-strip a:last-child {
		border-right: 0;
	}

	.activity-strip span {
		color: #687684;
		font-size: 0.72rem;
		font-weight: 750;
	}

	.activity-strip strong {
		color: #17202a;
		font-size: 1rem;
		line-height: 1;
	}

	.surface-row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.42rem 0.62rem;
		color: #62707d;
		font-size: 0.78rem;
		font-weight: 700;
	}

	.surface-row strong {
		margin-right: 0.18rem;
		color: #17202a;
		font-size: 0.82rem;
	}

	.card-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.62rem;
		margin-top: auto;
	}

	.card-actions a {
		color: #9f4820;
		font-size: 0.82rem;
		font-weight: 800;
		text-decoration: none;
	}

	.card-actions a:hover,
	.activity-strip a:hover,
	.grade:hover {
		text-decoration: underline;
	}

	@media (max-width: 1180px) {
		.protocol-grid {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}

	@media (max-width: 820px) {
		.page-shell {
			width: min(100vw - 1rem, var(--fw-page-max));
			padding-top: 0.8rem;
		}

		.page-top,
		.protocol-grid {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 440px) {
		.card-header,
		.activity-strip {
			grid-template-columns: 1fr;
		}

		.activity-strip a {
			border-right: 0;
			border-bottom: 1px solid rgba(23, 32, 42, 0.08);
		}

		.activity-strip a:last-child {
			border-bottom: 0;
		}
	}
</style>
