<script lang="ts">
	import { page } from '$app/state';
	import { protocolSlugAliases } from '$lib/catalog';
	import ProtocolActivityCard from '$lib/components/ProtocolActivityCard.svelte';
	import type { DefiProtocolDescriptor } from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	type ProtocolActivitySummary = {
		total: number;
		critical: number;
		high: number;
		standard: number;
		error: string | null;
	};

	type ProtocolRow = {
		protocol: DefiProtocolDescriptor;
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

	const rows = $derived.by<ProtocolRow[]>(() =>
		data.defiProtocols
			.map((protocol) => {
				const activity = data.protocolActivitySummaries[protocol.slug] ?? EMPTY_ACTIVITY;
				return {
					protocol,
					activity,
					searchText: protocolSearchText(protocol)
				};
			})
			.sort(protocolSort)
	);
	const normalizedSearch = $derived(normalizeSearch(searchQuery));
	const visibleRows = $derived.by(() =>
		rows.filter(
			(row) =>
				!normalizedSearch ||
				row.searchText.includes(normalizedSearch)
		)
	);
	const loadedActivityCount = $derived(
		rows.reduce((count, row) => count + (row.activity.error ? 0 : 1), 0)
	);

	function normalizeSearch(value: string) {
		return value.trim().toLowerCase();
	}

	function protocolSearchText(protocol: DefiProtocolDescriptor) {
		return [
			protocol.name,
			protocol.slug,
			...protocolSlugAliases(protocol.slug),
			...protocolSlugAliases(protocol.slug).map((slug) => slug.replaceAll('_', ' ')),
			protocol.program_id,
			...protocol.programs.flatMap((program) => [
				program.label,
				program.program_id,
				program.idl_metadata_address,
				program.anchor_legacy_idl_address ?? ''
			]),
			...(protocol.related_multisigs ?? []).flatMap((link) => [
				link.multisig,
				link.authority,
				link.program_id,
				link.program_label
			]),
			...protocol.sol_domains
		]
			.filter(Boolean)
			.join(' ')
			.toLowerCase();
	}

	function protocolSort(left: ProtocolRow, right: ProtocolRow) {
		return (
			right.activity.total - left.activity.total ||
			right.activity.critical - left.activity.critical ||
			left.protocol.name.localeCompare(right.protocol.name)
		);
	}

	function formatCount(value: number | null | undefined) {
		return new Intl.NumberFormat('en-US').format(value ?? 0);
	}

	function formatActivityCount(activity: ProtocolActivitySummary, value: number) {
		return activity.error ? '-' : formatCount(value);
	}

	function pluralLabel(count: number, singular: string, plural = `${singular}s`) {
		return `${formatCount(count)} ${count === 1 ? singular : plural}`;
	}

	function ruleCount(protocol: DefiProtocolDescriptor) {
		return protocol.amount_triggers.length + (protocol.coverage === 'admin_signals' ? 1 : 0);
	}

	function trackedSourceParts(protocol: DefiProtocolDescriptor) {
		const multisigCount = uniqueRelatedMultisigCount(protocol);
		const rules = ruleCount(protocol);
		return [
			protocol.sol_domains.length > 0 ? pluralLabel(protocol.sol_domains.length, 'wallet') : null,
			protocol.programs.length > 0 ? pluralLabel(protocol.programs.length, 'program') : null,
			multisigCount > 0 ? pluralLabel(multisigCount, 'multisig') : null,
			rules > 0 ? pluralLabel(rules, 'rule') : null
		].filter((part): part is string => Boolean(part));
	}

	function uniqueRelatedMultisigCount(protocol: DefiProtocolDescriptor) {
		return new Set((protocol.related_multisigs ?? []).map((link) => link.multisig).filter(Boolean)).size;
	}

	function trackedSourceLabel(protocol: DefiProtocolDescriptor) {
		const parts = trackedSourceParts(protocol);
		return parts.length > 0 ? parts.join(', ') : 'No tracked items';
	}

	function matchLabel(protocol: DefiProtocolDescriptor) {
		if (!normalizedSearch) return null;

		if (
			[protocol.name, protocol.slug, ...protocolSlugAliases(protocol.slug)].some((value) =>
				value.toLowerCase().includes(normalizedSearch)
			) ||
			protocolSlugAliases(protocol.slug).some((slug) =>
				slug.replaceAll('_', ' ').toLowerCase().includes(normalizedSearch)
			)
		) {
			return 'Protocol name';
		}

		const program = protocol.programs.find((candidate) =>
			[candidate.label, candidate.program_id, candidate.idl_metadata_address, candidate.anchor_legacy_idl_address]
				.filter(Boolean)
				.some((value) => value?.toLowerCase().includes(normalizedSearch))
		);
		if (program) return `Source: ${program.label}`;

		const domain = protocol.sol_domains.find((candidate) =>
			candidate.toLowerCase().includes(normalizedSearch)
		);
		if (domain) return `.sol name: ${domain}`;

		return null;
	}

	function protocolMatchLine(protocol: DefiProtocolDescriptor) {
		const match = matchLabel(protocol);
		return match ? `Match: ${match}` : null;
	}

	function protocolSummary(row: ProtocolRow) {
		if (row.activity.error) return `Activity unavailable: ${row.activity.error}`;
		if (row.activity.total === 0) return 'No indexed alerts for this protocol yet.';
		return `${formatCount(row.activity.total)} indexed alerts across monitored sources.`;
	}

	function protocolCardMetrics(row: ProtocolRow) {
		return [
			{ label: 'Total', value: formatActivityCount(row.activity, row.activity.total) },
			{ label: 'Critical', value: formatActivityCount(row.activity, row.activity.critical) },
			{ label: 'High', value: formatActivityCount(row.activity, row.activity.high) }
		];
	}

</script>

<svelte:head>
	<title>Protocols - FireWatch</title>
</svelte:head>

<div class="directory-shell">
	<section class="directory-top" aria-labelledby="protocols-title">
		<div class="title-panel">
			<h1 id="protocols-title">Protocols</h1>
			<p>Search the watchlist and open a protocol page to inspect live activity.</p>
		</div>
	</section>

	<section class="sticky-search-panel" aria-label="Search protocols">
		<label class="search-box">
			<span>Search protocols</span>
			<input
				bind:value={searchQuery}
				type="search"
				placeholder="Protocol, wallet, program address, .sol"
				autocomplete="off"
				spellcheck="false"
			/>
		</label>

		<div class="result-meta">
			<span>{formatCount(visibleRows.length)} of {formatCount(rows.length)} protocols</span>
			<span>{formatCount(loadedActivityCount)} activity indexes loaded</span>
		</div>
	</section>

	<section class="protocol-list-panel" aria-label="Protocol directory">
		{#if visibleRows.length === 0}
			<p class="empty-state">No protocol matched this search.</p>
		{:else}
			<div class="protocol-list">
				{#each visibleRows as row (row.protocol.slug)}
					<ProtocolActivityCard
						protocol={row.protocol}
						metrics={protocolCardMetrics(row)}
						sourceLabel={trackedSourceLabel(row.protocol)}
						latestLine={protocolSummary(row)}
						matchLine={protocolMatchLine(row.protocol)}
					/>
				{/each}
			</div>
		{/if}
	</section>
</div>

<style>
	.directory-shell {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem 0 2.4rem;
		display: grid;
		gap: 0.86rem;
	}

	.directory-top {
		display: grid;
		grid-template-columns: 1fr;
		gap: 0.86rem;
		align-items: stretch;
	}

	.title-panel,
	.sticky-search-panel,
	.protocol-list-panel {
		border: 1px solid var(--fw-border);
		border-radius: 8px;
		background: #fff;
	}

	.title-panel {
		display: grid;
		gap: 0.78rem;
		align-content: start;
		padding: 1rem;
	}

	.sticky-search-panel {
		position: sticky;
		top: 3.18rem;
		z-index: 20;
		display: grid;
		grid-template-columns: minmax(16rem, 1fr) auto;
		align-items: end;
		gap: 0.7rem;
		padding: 0.7rem 1rem;
		box-shadow: 0 1px 0 rgba(23, 32, 42, 0.04);
	}

	h1,
	p {
		margin: 0;
	}

	h1 {
		font-size: 2.2rem;
		line-height: 1;
		letter-spacing: 0;
	}

	p,
	.empty-state {
		color: var(--fw-muted);
		line-height: 1.5;
	}

	.search-box {
		display: grid;
		gap: 0.32rem;
		color: #596775;
		font-size: 0.78rem;
		font-weight: 760;
	}

	.search-box input {
		width: 100%;
		min-height: 2.46rem;
		padding: 0.48rem 0.62rem;
		border: 1px solid rgba(23, 32, 42, 0.14);
		border-radius: 7px;
		background: #fff;
		color: var(--fw-ink);
		font: inherit;
		font-size: 0.9rem;
		font-weight: 650;
		outline: none;
	}

	.search-box input:focus {
		border-color: rgba(173, 78, 34, 0.48);
		box-shadow: 0 0 0 3px rgba(173, 78, 34, 0.1);
	}

	.result-meta {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: 0.36rem 0.7rem;
		color: var(--fw-muted);
		font-size: 0.78rem;
		font-weight: 700;
		text-align: right;
	}

	.protocol-list-panel {
		padding: 0 1rem;
	}

	.protocol-list {
		display: grid;
	}

	.empty-state {
		padding: 1rem 0;
	}

	@media (max-width: 900px) {
		.directory-top {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 560px) {
		.directory-shell {
			width: min(100vw - 1rem, var(--fw-page-max));
			padding-top: 0.8rem;
		}

		h1 {
			font-size: 1.9rem;
		}

		.protocol-list-panel {
			padding: 0 0.8rem;
		}

		.sticky-search-panel {
			top: 2.98rem;
			grid-template-columns: 1fr;
			align-items: stretch;
			padding: 0.64rem 0.8rem;
		}

		.result-meta {
			justify-content: flex-start;
			text-align: left;
		}

	}
</style>
