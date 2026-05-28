<script lang="ts">
	import { programPageHref, protocolPageHref } from '$lib/catalog';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import { formatTimestamp, shortAddress } from '$lib/format';
	import { programCriticalHref, programRiskSummary, riskGradeTone } from '$lib/risk';
	import type {
		DefiProtocolDescriptor,
		ProtocolCoverage,
		ProtocolProgramSurface
	} from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	type ProgramFilter = 'all' | 'diffable' | 'idl' | ProtocolCoverage;
	const filterOptions: ProgramFilter[] = [
		'all',
		'diffable',
		'idl',
		'deep_decode',
		'admin_signals',
		'address_only'
	];
	let query = $state('');
	let filter = $state<ProgramFilter>('all');
	let rows = $derived.by(() =>
		data.defiProtocols
			.flatMap((protocol) =>
				protocol.programs.map((program) => ({
					protocol,
					program
				}))
			)
			.sort(
				(left, right) =>
					left.protocol.name.localeCompare(right.protocol.name) ||
					left.program.label.localeCompare(right.program.label)
			)
	);
	let summaries = $derived(Object.values(data.programVersionSummaries));
	let normalizedSearch = $derived(query.trim().toLowerCase());
	let visibleRows = $derived.by(() =>
		rows.filter((row) => matchesFilter(row) && matchesQuery(row))
	);
	let inventoryTotals = $derived.by(() => ({
		programs: rows.length,
		protocols: data.defiProtocols.length,
		withVersions: summaries.filter((summary) => summary.version_count > 0).length,
		versions: summaries.reduce((count, summary) => count + summary.version_count, 0),
		diffs: summaries.reduce((count, summary) => count + summary.diffable_count, 0),
		errors: summaries.filter((summary) => summary.error).length
	}));

	type ProgramRow = {
		protocol: DefiProtocolDescriptor;
		program: ProtocolProgramSurface;
	};

	function versionSummary(programId: string) {
		return data.programVersionSummaries[programId] ?? null;
	}

	function matchesFilter(row: ProgramRow) {
		if (filter === 'all') return true;
		if (filter === 'diffable') {
			return (versionSummary(row.program.program_id)?.diffable_count ?? 0) > 0;
		}
		if (filter === 'idl') {
			return Boolean(row.program.idl_metadata_address || row.program.anchor_legacy_idl_address);
		}
		return row.protocol.coverage === filter;
	}

	function matchesQuery(row: ProgramRow) {
		const value = normalizedSearch;
		if (!value) return true;
		return [
			row.protocol.name,
			row.protocol.slug,
			row.program.label,
			row.program.program_id,
			row.program.idl_metadata_address,
			row.program.anchor_legacy_idl_address ?? ''
		]
			.join(' ')
			.toLowerCase()
			.includes(value);
	}

	function formatCount(value: number | null | undefined) {
		return new Intl.NumberFormat('en-US').format(value ?? 0);
	}

	function formatByteCount(value: number | null | undefined) {
		if (value == null) return 'Unknown size';
		return `${formatCount(value)} bytes`;
	}

	function versionKindLabel(kind: string | undefined) {
		if (kind === 'program_deployed') return 'Deploy';
		if (kind === 'program_upgraded') return 'Upgrade';
		return kind?.replaceAll('_', ' ') ?? 'No version';
	}

	function latestDiffHref(row: ProgramRow) {
		const summary = versionSummary(row.program.program_id);
		if (!summary?.latest?.complete || !summary.latest.previous_version_id) return null;
		return `/programs/${row.program.program_id}/versions/${encodeURIComponent(summary.latest.id)}/compare`;
	}

	function filterLabel(nextFilter: ProgramFilter) {
		if (nextFilter === 'all') return 'All';
		if (nextFilter === 'diffable') return 'Has diff';
		if (nextFilter === 'idl') return 'IDL';
		if (nextFilter === 'deep_decode') return 'Deep decode';
		if (nextFilter === 'admin_signals') return 'Admin signals';
		return 'Address only';
	}
</script>

<svelte:head>
	<title>Tracked programs · FireWatch</title>
</svelte:head>

<div class="page-shell">
	<section class="panel page-top">
		<SectionHeader
			title="Tracked programs"
			description="Every cataloged program FireWatch watches for upgrades, IDL drift, signer activity, and code diffs."
			level="1"
		/>

		<div class="stats-strip" aria-label="Program inventory">
			<span class="stat-item">
				<strong>{formatCount(inventoryTotals.programs)}</strong>
				Programs
			</span>
			<span class="stat-item">
				<strong>{formatCount(inventoryTotals.protocols)}</strong>
				Protocols
			</span>
			<span class="stat-item">
				<strong>{formatCount(inventoryTotals.withVersions)}</strong>
				With versions
			</span>
			<span class="stat-item">
				<strong>{formatCount(inventoryTotals.versions)}</strong>
				Versions
			</span>
			<span class="stat-item">
				<strong>{formatCount(inventoryTotals.diffs)}</strong>
				Code diffs
			</span>
			{#if inventoryTotals.errors > 0}
				<span class="stat-item warning">
					<strong>{formatCount(inventoryTotals.errors)}</strong>
					Backend misses
				</span>
			{/if}
		</div>
	</section>

	<section class="panel inventory-panel">
		<div class="toolbar">
			<label class="search-field">
				<span>Search</span>
				<input
					type="search"
					bind:value={query}
					placeholder="Protocol, label, or address"
					autocomplete="off"
				/>
			</label>

			<div class="filter-tabs" aria-label="Program filters">
				{#each filterOptions as nextFilter}
					<button
						class:active={filter === nextFilter}
						type="button"
						onclick={() => (filter = nextFilter)}
					>
						{filterLabel(nextFilter)}
					</button>
				{/each}
			</div>
		</div>

		<div class="result-head">
			<span>{formatCount(visibleRows.length)} shown</span>
			<span>{filterLabel(filter)}</span>
		</div>

		{#if visibleRows.length === 0}
			<p class="empty-state">No tracked program matches this view.</p>
		{:else}
			<div class="program-list" role="list">
				{#each visibleRows as row}
					{@const summary = versionSummary(row.program.program_id)}
					{@const risk = programRiskSummary(row.program)}
					{@const diffHref = latestDiffHref(row)}
					<article class="program-row" role="listitem">
						<div class="program-main">
							<ProtocolIdentity
								slug={row.protocol.slug}
								label={row.protocol.name}
								href={protocolPageHref(row.protocol.slug)}
								size="sm"
							/>
							<a class="program-title" href={programPageHref(row.program.program_id)}>
								{row.program.label}
							</a>
							<code>{shortAddress(row.program.program_id)}</code>
						</div>

						<a class="risk-strip" href={programCriticalHref(row.program.program_id)}>
							<span class={`risk-grade grade-${riskGradeTone(risk)}`}>Rug {risk.grade}</span>
							<span>Score <strong>{risk.score}</strong></span>
							<span>Critical <strong>{risk.critical_total}</strong></span>
						</a>

						<div class="version-cell">
							{#if summary?.error}
								<span class="version-error">Versions unavailable</span>
							{:else if summary?.latest}
								<strong>{versionKindLabel(summary.latest.kind)} at slot {formatCount(summary.latest.slot)}</strong>
								<span>{formatTimestamp(summary.latest.block_time)} - {formatByteCount(summary.latest.program_len)}</span>
								<span>{formatCount(summary.version_count)} versions - {formatCount(summary.diffable_count)} diffs</span>
							{:else}
								<strong>No extracted versions</strong>
								<span>Waiting for a loader upload.</span>
							{/if}
						</div>

						<div class="row-actions">
							<a class="primary-link compact-link" href={programPageHref(row.program.program_id)}>
								Versions
							</a>
							{#if diffHref}
								<a class="secondary-link compact-link" href={diffHref}>Latest diff</a>
							{/if}
						</div>
					</article>
				{/each}
			</div>
		{/if}
	</section>
</div>

<style>
	.page-shell {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem 0 2.4rem;
		display: grid;
		gap: 0.76rem;
	}

	.panel {
		background: #fff;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 10px;
		padding: 0.96rem 1rem;
	}

	.page-top,
	.inventory-panel {
		display: grid;
		gap: 0.76rem;
	}

	.stats-strip {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(126px, 1fr));
		gap: 1px;
		overflow: hidden;
		padding: 0;
		border-radius: 8px;
		background: var(--fw-border);
	}

	.stat-item {
		display: inline-flex;
		flex-direction: column;
		gap: 0.06rem;
		min-width: 0;
		padding: 0.62rem 0.68rem;
		background: var(--fw-panel-soft);
		color: #62707d;
		font-size: 0.76rem;
		font-weight: 600;
	}

	.stat-item strong {
		color: #17202a;
		font-size: 0.94rem;
	}

	.stat-item.warning strong {
		color: #9c3929;
	}

	.toolbar,
	.result-head {
		display: flex;
		align-items: end;
		justify-content: space-between;
		gap: 0.76rem;
		flex-wrap: wrap;
	}

	.search-field {
		display: grid;
		gap: 0.22rem;
		flex: 1 1 18rem;
		min-width: min(100%, 18rem);
	}

	.search-field span,
	.result-head {
		color: #697786;
		font-size: 0.78rem;
		font-weight: 700;
	}

	.search-field input {
		width: 100%;
		min-height: 2.42rem;
		padding: 0.56rem 0.68rem;
		border: 1px solid rgba(23, 32, 42, 0.12);
		border-radius: 8px;
		background: #fff;
		color: #17202a;
		font: inherit;
		font-size: 0.9rem;
	}

	.filter-tabs,
	.row-actions {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0.46rem;
	}

	.filter-tabs button,
	.primary-link,
	.secondary-link {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border-radius: 8px;
		font-weight: 700;
		text-decoration: none;
		cursor: pointer;
	}

	.filter-tabs button {
		min-height: 2.42rem;
		padding: 0.5rem 0.68rem;
		border: 1px solid rgba(23, 32, 42, 0.12);
		background: #fff;
		color: #556473;
		font-size: 0.82rem;
	}

	.filter-tabs button.active {
		color: #fff;
		border-color: #bd4a17;
		background: #bd4a17;
	}

	.program-list {
		display: grid;
		gap: 1px;
		overflow: hidden;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		background: rgba(23, 32, 42, 0.08);
	}

	.program-row {
		display: grid;
		grid-template-columns: minmax(220px, 1.1fr) minmax(220px, 0.95fr) minmax(260px, 1fr) auto;
		align-items: center;
		gap: 0.78rem;
		min-width: 0;
		padding: 0.72rem 0.78rem;
		background: #fff;
	}

	.program-main,
	.version-cell {
		display: grid;
		gap: 0.16rem;
		min-width: 0;
	}

	.program-title {
		color: #17202a;
		font-size: 0.94rem;
		font-weight: 800;
		text-decoration: none;
		overflow-wrap: anywhere;
	}

	.program-title:hover,
	.program-title:focus-visible {
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.risk-strip {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.36rem;
		color: #657381;
		text-decoration: none;
		font-size: 0.75rem;
		font-weight: 700;
	}

	.risk-strip span {
		display: inline-flex;
		align-items: center;
		gap: 0.2rem;
		min-height: 1.38rem;
		padding: 0.14rem 0.36rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 6px;
		background: #fbfaf8;
	}

	.risk-strip strong {
		color: #17202a;
	}

	.risk-grade {
		color: #216d4d;
	}

	.risk-grade.grade-c,
	.risk-grade.grade-d {
		color: #8b5b12;
	}

	.risk-grade.grade-f {
		color: #9c3929;
	}

	.version-cell strong {
		color: #1d2730;
		font-size: 0.86rem;
	}

	.version-cell span,
	.version-error,
	code,
	.empty-state {
		color: #66737f;
		font-size: 0.78rem;
		font-weight: 600;
	}

	.version-error {
		color: #9c3929;
	}

	code {
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
		overflow-wrap: anywhere;
	}

	.primary-link,
	.secondary-link {
		border: 1px solid rgba(23, 32, 42, 0.12);
		font-size: 0.8rem;
	}

	.compact-link {
		min-height: 2.1rem;
		padding: 0.48rem 0.62rem;
		white-space: nowrap;
	}

	.primary-link {
		border-color: #bd4a17;
		background: #bd4a17;
		color: white;
	}

	.secondary-link {
		background: #fff;
		color: #556473;
	}

	.empty-state {
		margin: 0;
		padding: 0.74rem 0.82rem;
		border-radius: 8px;
		background: #fbfaf8;
	}

	@media (max-width: 1040px) {
		.program-row {
			grid-template-columns: minmax(0, 1fr) minmax(220px, 0.9fr);
			align-items: start;
		}

		.row-actions {
			justify-content: flex-start;
		}
	}

	@media (max-width: 720px) {
		.page-shell {
			width: min(100vw - 1rem, var(--fw-page-max));
			padding-top: 0.8rem;
		}

		.program-row {
			grid-template-columns: 1fr;
		}

		.filter-tabs {
			width: 100%;
		}

		.filter-tabs button {
			flex: 1 1 8rem;
		}
	}
</style>
