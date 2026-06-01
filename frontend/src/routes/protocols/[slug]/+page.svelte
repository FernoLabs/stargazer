<script lang="ts">
	import { explorerAddressUrl, explorerIdlUrl } from '$lib/explorer';
	import { nextPageOffset } from '$lib/api-pagination';
	import { programPageHref, protocolActivityPageHref, syntheticSquadsDescriptor } from '$lib/catalog';
	import ActivityEventRow from '$lib/components/ActivityEventRow.svelte';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import { shortAddress } from '$lib/format';
	import { programCriticalHref, programRiskSummary, projectRiskSummary } from '$lib/risk';
	import type { ChainEvent, DefiProtocolDescriptor, EventsResponse, ProgramRiskSummary } from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	let appendedEvents = $state<ChainEvent[]>([]);
	let nextOffsetOverride = $state<number | null | undefined>(undefined);
	let loadingMore = $state(false);
	let loadMoreError = $state<string | null>(null);

	type ProtocolRiskRow = {
		program: DefiProtocolDescriptor['programs'][number];
		risk: ProgramRiskSummary;
	};

	type ProjectRiskSnapshot = ProgramRiskSummary & { programs: number };

	function protocol() {
		return data.protocol as DefiProtocolDescriptor | null;
	}

	function formatCount(value: number | null | undefined) {
		return new Intl.NumberFormat('en-US').format(value ?? 0);
	}

	function catalogProtocols() {
		const current = protocol();
		if (!current) return [syntheticSquadsDescriptor()];
		if (current.slug === 'squads') return [current];
		return [current, syntheticSquadsDescriptor()];
	}

	function allEvents() {
		if (appendedEvents.length === 0) return data.events;
		const seen = new Set(data.events.map((event) => event.id));
		return [...data.events, ...appendedEvents.filter((event) => !seen.has(event.id))];
	}

	function currentNextOffset() {
		return nextOffsetOverride === undefined ? data.nextOffset : nextOffsetOverride;
	}

	function payloadString(event: ChainEvent, key: string) {
		const value = event.payload[key];
		return typeof value === 'string' && value.length > 0 ? value : null;
	}

	function protocolMultisigs() {
		const addresses = new Set<string>();
		for (const event of allEvents()) {
			if (event.target_type === 'multisig') {
				addresses.add(event.target_address);
			}
			const payloadMultisig = payloadString(event, 'multisig');
			if (payloadMultisig) {
				addresses.add(payloadMultisig);
			}
		}
		return [...addresses];
	}

	function programSurfaceRows(): ProtocolRiskRow[] {
		return (protocol()?.programs ?? [])
			.map((program) => ({ program, risk: programRiskSummary(program) }))
			.sort((left, right) => {
				if (left.risk.unreviewed_critical !== right.risk.unreviewed_critical) {
					return right.risk.unreviewed_critical - left.risk.unreviewed_critical;
				}
				if (left.risk.critical_total !== right.risk.critical_total) {
					return right.risk.critical_total - left.risk.critical_total;
				}
				return left.program.label.localeCompare(right.program.label);
			});
	}

	function protocolRiskSnapshot(): ProjectRiskSnapshot {
		const currentProtocol = protocol();
		return {
			...projectRiskSummary(currentProtocol),
			programs: currentProtocol?.programs.length ?? 0
		};
	}

	function stats() {
		const current = protocol();
		const snapshot = protocolRiskSnapshot();
		return [
			{ label: 'Project grade', value: snapshot.grade },
			{ label: 'Project score', value: formatCount(snapshot.score) },
			{ label: 'Activity events', value: formatCount(data.protocolEventTotalCount) },
			{ label: 'Critical alerts', value: formatCount(data.protocolAlertCounts.critical) },
			{ label: 'High alerts', value: formatCount(data.protocolAlertCounts.high) },
			{ label: 'Tracked programs', value: formatCount(current?.programs.length ?? 0) },
			{ label: 'Related multisigs', value: formatCount(protocolMultisigs().length) }
		];
	}

	function trackedSurfaceStats() {
		const current = protocol();
		return [
			{ label: 'Programs', value: formatCount(current?.programs.length ?? 0) },
			{ label: '.sol domains', value: formatCount(current?.sol_domains.length ?? 0) },
			{ label: 'Amount rules', value: formatCount(current?.amount_triggers.length ?? 0) },
			{ label: 'Related multisigs', value: formatCount(protocolMultisigs().length) }
		];
	}

	async function loadMore() {
		const offset = currentNextOffset();
		if (loadingMore || offset === null) return;

		loadingMore = true;
		loadMoreError = null;

		try {
			const search = new URLSearchParams({ 'page[size]': '40', 'page[offset]': String(offset) });
			if (protocol()) {
				search.set('protocol', protocol()!.slug);
				search.set('include_related_multisigs', 'true');
			}

			const response = await fetch(`${data.backendBaseUrl}/api/events?${search.toString()}`);
			if (!response.ok) throw new Error(`Backend returned ${response.status}`);

			const payload = (await response.json()) as EventsResponse;
			const seenIds = new Set(allEvents().map((event) => event.id));
			appendedEvents = [...appendedEvents, ...payload.items.filter((event) => !seenIds.has(event.id))];
			nextOffsetOverride = nextPageOffset(payload.links);
		} catch (error) {
			loadMoreError = error instanceof Error ? error.message : 'Unknown backend error';
		} finally {
			loadingMore = false;
		}
	}
</script>

<svelte:head>
	<title>{`${protocol()?.name ?? 'Protocol'} · FireWatch`}</title>
</svelte:head>

<div class="page-shell">
	<section class="panel page-top">
		<div class="title-row">
			<ProtocolIdentity slug={protocol()?.slug ?? null} label={protocol()?.name ?? 'Protocol'} />
			<div class="title-copy">
				<h1>{protocol()?.name}</h1>
				<p class="lede">
					Project-grade monitoring with per-program proof, tracked surfaces, and protocol activity.
				</p>
			</div>
		</div>

		<div class="page-actions">
			<a class="primary-link" href={protocolActivityPageHref(protocol()?.slug ?? '')}>Protocol activity</a>
		</div>

		<div class="stats-strip">
			{#each stats() as stat (stat.label)}
				<span class="stat-item">
					<strong>{stat.value}</strong>
					{stat.label}
				</span>
			{/each}
		</div>

		{#if data.protocolEventsError}
			<p class="flash error">Activity unavailable: {data.protocolEventsError}</p>
		{/if}
	</section>

	<section class="panel score-panel">
		{#if protocol()}
			{@const snapshot = protocolRiskSnapshot()}
			<div class="score-head">
				<div>
					<h2>Protocol score overview</h2>
					<p>Grade is assigned at the project level from tracked program proof and alert review state.</p>
				</div>
			</div>

			<div class="score-summary">
				<div class={`grade-box grade-${snapshot.grade.toLowerCase()}`}>
					<span>Grade</span>
					<strong>{snapshot.grade}</strong>
				</div>
				<div class="score-copy">
					<strong>Project score {snapshot.score}</strong>
					<p>Across {formatCount(snapshot.programs)} tracked programs in this protocol.</p>
					<div class="score-metrics">
						<span>Critical <strong>{formatCount(snapshot.critical_total)}</strong></span>
						<span>Unresolved <strong>{formatCount(snapshot.unreviewed_critical)}</strong></span>
						<span>Reviewed <strong>{formatCount(snapshot.reviewed_critical)}</strong></span>
					</div>
				</div>
			</div>
		{/if}
	</section>

	<section class="panel tracked-panel">
		<SectionHeader
			title="Tracked surfaces"
			description="Programs, .sol domains, amount rules, and related multisigs currently attached to this protocol."
			compact
		/>

		<div class="surface-summary" aria-label="Tracked surface counts">
			{#each trackedSurfaceStats() as item (item.label)}
				<span>
					<strong>{item.value}</strong>
					{item.label}
				</span>
			{/each}
		</div>

		<div class="surface-list">
			{#if programSurfaceRows().length > 0}
				<div class="surface-group">
					<div class="surface-group-head">
						<h3>Programs</h3>
						<p>Program accounts watched for upgrades, IDL changes, and score proof.</p>
					</div>
					{#each programSurfaceRows() as row (row.program.program_id)}
						<div class="surface-row">
							<div class="surface-copy">
								<a class="surface-name" href={programPageHref(row.program.program_id)}>{row.program.label}</a>
								<code>{shortAddress(row.program.program_id)}</code>
								<div class="risk-strip compact">
									<span>Critical <strong>{row.risk.critical_total}</strong></span>
									<span>Unresolved <strong>{row.risk.unreviewed_critical}</strong></span>
									<span>Reviewed <strong>{row.risk.reviewed_critical}</strong></span>
								</div>
							</div>
							<div class="surface-links">
								<a href={programCriticalHref(row.program.program_id)}>Proof</a>
								<a href={explorerAddressUrl(row.program.program_id, data.explorerCluster)} rel="noreferrer" target="_blank">
									Explorer
								</a>
								{#if row.program.idl_metadata_address}
									<a href={explorerIdlUrl(row.program.program_id, data.explorerCluster)} rel="noreferrer" target="_blank">
										IDL
									</a>
								{/if}
								<a href={programPageHref(row.program.program_id)}>Open</a>
							</div>
						</div>
					{/each}
				</div>
			{/if}

			{#if (protocol()?.sol_domains.length ?? 0) > 0}
				<div class="surface-group">
					<div class="surface-group-head">
						<h3>.sol domains</h3>
						<p>SNS names attached to the protocol watch surface.</p>
					</div>
					{#each protocol()?.sol_domains ?? [] as domain (domain)}
						<div class="surface-row">
							<div class="surface-copy">
								<span class="surface-name">{domain}</span>
								<code>SNS root domain</code>
							</div>
							<div class="surface-links">
								<a href={protocolActivityPageHref(protocol()?.slug ?? '')}>Activity</a>
							</div>
						</div>
					{/each}
				</div>
			{/if}

			{#if (protocol()?.amount_triggers.length ?? 0) > 0}
				<div class="surface-group">
					<div class="surface-group-head">
						<h3>Amount rules</h3>
						<p>Value thresholds applied to decoded instructions and transfers.</p>
					</div>
					{#each protocol()?.amount_triggers ?? [] as trigger (`${trigger.label}:${trigger.description}`)}
						<div class="surface-row">
							<div class="surface-copy">
								<span class="surface-name">{trigger.label}</span>
								<p>{trigger.description}</p>
							</div>
							<div class="surface-links">
								<span class="surface-tag">Amount trigger</span>
							</div>
						</div>
					{/each}
				</div>
			{/if}

			{#if protocolMultisigs().length > 0}
				<div class="surface-group">
					<div class="surface-group-head">
						<h3>Related multisigs</h3>
						<p>Governance and signer accounts inferred from indexed protocol activity.</p>
					</div>
					{#each protocolMultisigs() as multisig (multisig)}
						<div class="surface-row">
							<div class="surface-copy">
								<span class="surface-name">{shortAddress(multisig)}</span>
								<code>{multisig}</code>
							</div>
							<div class="surface-links">
								<a href={explorerAddressUrl(multisig, data.explorerCluster)} rel="noreferrer" target="_blank">
									Explorer
								</a>
								<a href={protocolActivityPageHref(protocol()?.slug ?? '')}>Activity</a>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>

		{#if (protocol()?.programs.length ?? 0) === 0 && (protocol()?.sol_domains.length ?? 0) === 0 && (protocol()?.amount_triggers.length ?? 0) === 0 && protocolMultisigs().length === 0}
			<p class="inline-note">No monitor surfaces are attached to this protocol yet.</p>
		{/if}
	</section>

	<section class="panel">
		<SectionHeader
			title="Protocol activity"
			description={`Showing ${formatCount(allEvents().length)} of ${formatCount(data.protocolEventTotalCount)} domain, program, and multisig events for this protocol.`}
			href={protocolActivityPageHref(protocol()?.slug ?? '')}
			hrefLabel="Open filters"
			compact
		/>

		{#if allEvents().length === 0}
			<p class="empty-state">No protocol events for this view yet.</p>
		{:else}
			<div class="event-list">
				{#each allEvents() as event (event.id)}
					<ActivityEventRow
						{event}
						explorerCluster={data.explorerCluster}
						protocols={catalogProtocols()}
						fallbackProtocolSlug={protocol()?.slug ?? null}
						fallbackProtocolLabel={protocol()?.name ?? null}
					/>
				{/each}
			</div>
		{/if}
	</section>

	{#if currentNextOffset() !== null}
		<div class="load-row">
			<button class="primary-link button-like" type="button" onclick={loadMore} disabled={loadingMore}>
				{loadingMore ? 'Loading…' : 'Load more'}
			</button>
			{#if loadMoreError}
				<p class="flash error">{loadMoreError}</p>
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
		gap: 0.76rem;
	}

	.panel {
		background: #fff;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 10px;
		padding: 0.96rem 1rem;
	}

	.page-top,
	.title-row,
	.title-copy,
	.surface-list,
	.event-list,
	.score-panel,
	.tracked-panel {
		display: grid;
		gap: 0.68rem;
	}

	.title-row {
		grid-template-columns: auto minmax(0, 1fr);
		align-items: start;
		min-width: 0;
	}

	h1,
	h2,
	h3,
	strong,
	p {
		margin: 0;
	}

	h1 {
		font-size: 1.85rem;
		line-height: 1.02;
		letter-spacing: 0;
		overflow-wrap: anywhere;
	}

	h2 {
		font-size: 1.06rem;
		line-height: 1.2;
	}

	h3 {
		font-size: 0.9rem;
		line-height: 1.25;
	}

	.lede,
	.inline-note,
	.empty-state,
	.flash,
	.score-head p,
	.score-copy p {
		color: #5f6c79;
	}

	.lede {
		max-width: 60ch;
		line-height: 1.5;
	}

	.page-actions,
	.surface-links,
	.load-row,
	.score-metrics,
	.risk-strip {
		display: flex;
		flex-wrap: wrap;
		gap: 0.52rem;
	}

	.stats-strip {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
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
		font-size: 0.76rem;
		font-weight: 600;
		color: #62707d;
	}

	.stat-item strong {
		color: #17202a;
		font-size: 0.9rem;
		overflow-wrap: anywhere;
	}

	.score-head {
		display: flex;
		justify-content: space-between;
		align-items: start;
		gap: 0.8rem;
	}

	.score-summary {
		display: grid;
		grid-template-columns: minmax(130px, 0.25fr) minmax(0, 1fr);
		gap: 0.8rem;
		align-items: stretch;
	}

	.grade-box {
		display: grid;
		align-content: center;
		gap: 0.08rem;
		padding: 0.74rem;
		border-radius: 8px;
		background: rgba(61, 140, 99, 0.12);
		color: #216d4d;
		text-align: center;
	}

	.grade-box span {
		font-size: 0.74rem;
		font-weight: 700;
	}

	.grade-box strong {
		font-size: 1.45rem;
	}

	.grade-box.grade-c,
	.grade-box.grade-d {
		background: rgba(179, 123, 36, 0.14);
		color: #8a5e1e;
	}

	.grade-box.grade-f {
		background: rgba(191, 74, 57, 0.14);
		color: #9c3929;
	}

	.score-copy {
		display: grid;
		gap: 0.3rem;
		align-content: center;
	}

	.score-metrics span {
		font-size: 0.77rem;
		font-weight: 700;
		color: #5c6a77;
	}

	.score-metrics strong {
		color: #17202a;
		font-size: 0.79rem;
	}

	.surface-group {
		display: grid;
		gap: 0.42rem;
	}

	.surface-group-head {
		display: grid;
		gap: 0.12rem;
		padding: 0.1rem 0.04rem;
	}

	.surface-group-head p {
		color: #5f6c79;
		font-size: 0.82rem;
		line-height: 1.42;
	}

	.surface-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: start;
		gap: 0.72rem;
		padding: 0.72rem 0.78rem;
		border-radius: 8px;
		background: #fbfaf8;
		border: 1px solid rgba(23, 32, 42, 0.08);
	}

	.surface-copy {
		display: grid;
		gap: 0.18rem;
		min-width: 0;
	}

	code {
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
		font-size: 0.8rem;
		color: #63707c;
		overflow-wrap: anywhere;
	}

	.surface-copy code {
		font-size: 0.76rem;
	}

	.surface-copy p {
		color: #5f6c79;
		font-size: 0.84rem;
		line-height: 1.45;
	}

	.risk-strip {
		gap: 0.34rem 0.48rem;
		color: #596775;
		text-decoration: none;
		align-items: center;
	}

	.risk-strip span {
		font-size: 0.74rem;
		font-weight: 700;
	}

	.risk-strip strong {
		color: #17202a;
		font-size: 0.78rem;
	}

	.risk-strip.compact {
		padding-top: 0.18rem;
	}

	.surface-name,
	.surface-links a {
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.surface-name {
		font-weight: 800;
		color: #1f2933;
	}

	.surface-links a {
		font-size: 0.82rem;
		font-weight: 800;
		color: #556473;
	}

	.surface-tag {
		display: inline-flex;
		align-items: center;
		min-height: 1.42rem;
		padding: 0.16rem 0.42rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 6px;
		background: #fbfaf8;
		color: #596775;
		font-size: 0.74rem;
		font-weight: 700;
	}

	.primary-link,
	.button-like {
		display: inline-flex;
		flex: 0 0 auto;
		align-items: center;
		justify-content: center;
		padding: 0.66rem 0.86rem;
		border-radius: 8px;
		font-size: 0.86rem;
		font-weight: 600;
		text-decoration: none;
	}

	.primary-link {
		border: 1px solid #bd4a17;
		background: #bd4a17;
		color: white;
	}

	.button-like {
		cursor: pointer;
	}

	.surface-summary {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: 1px;
		overflow: hidden;
		border-radius: 8px;
		background: var(--fw-border);
	}

	.surface-summary span {
		display: grid;
		gap: 0.06rem;
		min-width: 0;
		padding: 0.58rem 0.64rem;
		background: var(--fw-panel-soft);
		color: #62707d;
		font-size: 0.76rem;
		font-weight: 700;
	}

	.surface-summary strong {
		color: #17202a;
		font-size: 0.9rem;
	}

	.inline-note {
		padding: 0.74rem 0.8rem;
		border-radius: 8px;
		background: #fbfaf8;
		border: 1px solid rgba(23, 32, 42, 0.08);
	}

	.flash {
		padding: 0.74rem 0.82rem;
		border-radius: 8px;
		font-size: 0.88rem;
		font-weight: 600;
	}

	.flash.success {
		background: rgba(61, 140, 99, 0.14);
		color: #216d4d;
	}

	.flash.error {
		background: rgba(191, 74, 57, 0.14);
		color: #9c3929;
	}

	.load-row {
		align-items: center;
	}

	@media (max-width: 980px) {
		.score-summary,
		.surface-summary {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 720px) {
		.page-shell {
			width: min(100vw - 1rem, var(--fw-page-max));
			padding-top: 0.8rem;
		}

		.title-row {
			grid-template-columns: 1fr;
		}

		.surface-row {
			grid-template-columns: 1fr;
		}

		.score-head {
			display: grid;
		}

		.stat-item:last-child:nth-child(odd) {
			grid-column: 1 / -1;
		}
	}
</style>
