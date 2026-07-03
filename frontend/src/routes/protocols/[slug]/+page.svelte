<script lang="ts">
	import { resolve } from '$app/paths';
	import { nextPageOffset } from '$lib/api-pagination';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import SimpleEventFeed from '$lib/components/SimpleEventFeed.svelte';
	import { compactLongTokens } from '$lib/event-preview';
	import type { ChainEvent, DefiProtocolDescriptor, EventsResponse } from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	type MonitoredItem = {
		id: string;
		kind: string;
		title: string;
		detail: string;
		note: string;
		programAddress: string | null;
		monospace: boolean;
	};

	let appendedEvents = $state<ChainEvent[]>([]);
	let nextOffsetOverride = $state<number | null | undefined>(undefined);
	let loadingMore = $state(false);
	let loadMoreError = $state<string | null>(null);
	const monitoredItems = $derived.by(() => monitoredItemsFor(protocol()));

	const summaryMetrics = $derived.by(() => [
		{
			label: 'Alerts',
			value: data.protocolEventsError ? '-' : formatCount(data.protocolEventTotalCount)
		},
		{
			label: 'Critical',
			value: data.protocolEventsError ? '-' : formatCount(data.protocolAlertCounts.critical)
		},
		{
			label: 'High',
			value: data.protocolEventsError ? '-' : formatCount(data.protocolAlertCounts.high)
		},
		{
			label: 'Sources',
			value: formatCount(sourceCount(protocol()))
		}
	]);

	function protocol() {
		return data.protocol as DefiProtocolDescriptor | null;
	}

	function formatCount(value: number | null | undefined) {
		return new Intl.NumberFormat('en-US').format(value ?? 0);
	}

	function allEvents() {
		if (appendedEvents.length === 0) return data.events;
		const seen = new Set(data.events.map((event) => event.id));
		return [...data.events, ...appendedEvents.filter((event) => !seen.has(event.id))];
	}

	function currentNextOffset() {
		return nextOffsetOverride === undefined ? data.nextOffset : nextOffsetOverride;
	}

	function feedProtocols() {
		const current = protocol();
		return current ? [current] : data.defiProtocols;
	}

	function sourceCount(current: DefiProtocolDescriptor | null) {
		return monitoredItemsFor(current).length;
	}

	function monitoredItemsFor(current: DefiProtocolDescriptor | null): MonitoredItem[] {
		if (!current) return [];

		const programItems =
			current.programs.length > 0
				? current.programs.map((program) => ({
						id: `program:${program.program_id}`,
						kind: 'Program',
						title: program.label,
						detail: program.program_id,
						note: program.idl_metadata_address ? 'Program and IDL changes' : 'Program changes',
						programAddress: program.program_id,
						monospace: true
					}))
				: current.program_id
					? [
							{
								id: `program:${current.program_id}`,
								kind: 'Program',
								title: 'Main program',
								detail: current.program_id,
								note: 'Program changes',
								programAddress: current.program_id,
								monospace: true
							}
						]
					: [];

		return [
			...programItems,
			...monitoredMultisigItems(current),
			...current.sol_domains.map((domain) => ({
				id: `domain:${domain}`,
				kind: 'Wallet',
				title: domain,
				detail: 'Wallet or .sol domain activity',
				note: 'Address activity',
				programAddress: null,
				monospace: false
			})),
			...current.amount_triggers.map((trigger, index) => ({
				id: `rule:${index}:${trigger.label}`,
				kind: 'Rule',
				title: trigger.label,
				detail: trigger.description,
				note: 'Value threshold',
				programAddress: null,
				monospace: false
			}))
		];
	}

	function monitoredMultisigItems(current: DefiProtocolDescriptor) {
		const groups: {
			key: string;
			link: NonNullable<DefiProtocolDescriptor['related_multisigs']>[number];
			programLabels: string[];
		}[] = [];

		for (const link of current.related_multisigs ?? []) {
			const key = `${link.multisig}:${link.authority}:${link.vault_index ?? 'unknown'}`;
			let group = groups.find((candidate) => candidate.key === key);
			if (!group) {
				group = { key, link, programLabels: [] };
				groups.push(group);
			}
			if (link.program_label && !group.programLabels.includes(link.program_label)) {
				group.programLabels.push(link.program_label);
			}
		}

		return groups.map(({ link, programLabels }) => ({
			id: `multisig:${link.multisig}:${link.authority}:${link.vault_index ?? 'unknown'}`,
			kind: 'Multisig',
			title: link.vault_index === null ? 'Squads multisig' : `Squads vault ${link.vault_index}`,
			detail: link.multisig,
			note:
				programLabels.length === 1
					? `${programLabels[0]} upgrade control`
					: `Upgrade control for ${formatCount(programLabels.length)} programs`,
			programAddress: null,
			monospace: true
		}));
	}

	function monitoringSummary(current: DefiProtocolDescriptor | null) {
		if (!current) return 'No monitored items';
		const multisigCount = uniqueRelatedMultisigCount(current);
		const parts = [
			current.programs.length > 0
				? `${formatCount(current.programs.length)} program${current.programs.length === 1 ? '' : 's'}`
				: null,
			multisigCount > 0
				? `${formatCount(multisigCount)} multisig${multisigCount === 1 ? '' : 's'}`
				: null,
			current.sol_domains.length > 0
				? `${formatCount(current.sol_domains.length)} wallet${current.sol_domains.length === 1 ? '' : 's'}`
				: null,
			current.amount_triggers.length > 0
				? `${formatCount(current.amount_triggers.length)} rule${current.amount_triggers.length === 1 ? '' : 's'}`
				: null
		].filter((part): part is string => Boolean(part));

		return parts.length > 0 ? parts.join(' · ') : 'No monitored items';
	}

	function uniqueRelatedMultisigCount(current: DefiProtocolDescriptor) {
		return new Set((current.related_multisigs ?? []).map((link) => link.multisig).filter(Boolean)).size;
	}

	function coverageLabel(current: DefiProtocolDescriptor | null) {
		if (!current) return 'Protocol watch';
		if (current.coverage === 'deep_decode') return 'Decoded alerts';
		if (current.coverage === 'admin_signals') return 'Admin-change alerts';
		return 'Address watch';
	}

	function summaryLine() {
		if (data.protocolEventsError) return 'Activity is temporarily unavailable.';
		if (data.protocolEventTotalCount === 0) return 'No indexed alerts for this protocol yet.';
		if (data.protocolAlertCounts.critical > 0) {
			return `${formatCount(data.protocolAlertCounts.critical)} critical alerts need attention.`;
		}
		if (data.protocolAlertCounts.high > 0) {
			return `${formatCount(data.protocolAlertCounts.high)} high-priority alerts are indexed.`;
		}
		return 'Recent activity is normal.';
	}

	function latestEvent() {
		return allEvents()[0] ?? null;
	}

	function displayText(value: string) {
		return compactLongTokens(value);
	}

	async function loadMore() {
		const offset = currentNextOffset();
		if (loadingMore || offset === null) return;

		loadingMore = true;
		loadMoreError = null;

		try {
			const search = new URLSearchParams({ 'page[size]': '40', 'page[offset]': String(offset) });
			const current = protocol();
			if (current) {
				search.set('protocol', current.slug);
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
	<title>{`${protocol()?.name ?? 'Protocol'} - FireWatch`}</title>
</svelte:head>

<div class="protocol-shell">
	<a class="back-link" href={resolve('/protocols')} aria-label="Back to protocols">
		<svg aria-hidden="true" viewBox="0 0 20 20" focusable="false">
			<path d="M12.5 4.5 7 10l5.5 5.5" />
		</svg>
		<span>Back to protocols</span>
	</a>

	<section class="protocol-top" aria-labelledby="protocol-title">
		<div class="identity-line">
			<ProtocolIdentity slug={protocol()?.slug ?? null} label={protocol()?.name ?? 'Protocol'} />
			<span>{coverageLabel(protocol())}</span>
		</div>

		<div class="title-row">
			<div class="title-copy">
				<h1 id="protocol-title">{protocol()?.name}</h1>
				<p>{summaryLine()}</p>
			</div>

			<div class="page-actions">
				{#if protocol()}
					<a class="primary-action" href={resolve('/protocols/[slug]/activity', { slug: protocol()!.slug })}>Open feed</a>
				{:else}
					<a class="primary-action" href={resolve('/events')}>Open feed</a>
				{/if}
			</div>
		</div>

		<div class="summary-grid" aria-label="Protocol alert summary">
			{#each summaryMetrics as metric (metric.label)}
				<div class="summary-card">
					<strong>{metric.value}</strong>
					<span>{metric.label}</span>
				</div>
			{/each}
		</div>

		{#if data.protocolEventsError}
			<p class="flash error">Activity unavailable: {data.protocolEventsError}</p>
		{/if}
	</section>

	<div class="detail-grid">
		<section class="panel monitoring-panel" aria-labelledby="monitoring-title">
			<div class="panel-head">
				<div>
					<h2 id="monitoring-title">Monitored items</h2>
					<p>{monitoringSummary(protocol())}</p>
				</div>
			</div>

			{#if monitoredItems.length === 0}
				<p class="empty-state">No monitored programs, multisigs, wallets, or rules are listed for this protocol yet.</p>
			{:else}
				<div class="monitoring-list">
					{#each monitoredItems as item (item.id)}
						{#if item.programAddress}
							<a
								class="monitoring-row"
								href={resolve('/programs/[address]', { address: item.programAddress })}
							>
								<span class="monitoring-kind">{item.kind}</span>
								<span class="monitoring-copy">
									<strong>{item.title}</strong>
									{#if item.monospace}
										<code>{displayText(item.detail)}</code>
									{:else}
										<span>{displayText(item.detail)}</span>
									{/if}
								</span>
								<span class="monitoring-note">{item.note}</span>
							</a>
						{:else}
							<div class="monitoring-row">
								<span class="monitoring-kind">{item.kind}</span>
								<span class="monitoring-copy">
									<strong>{item.title}</strong>
									{#if item.monospace}
										<code>{displayText(item.detail)}</code>
									{:else}
										<span>{displayText(item.detail)}</span>
									{/if}
								</span>
								<span class="monitoring-note">{item.note}</span>
							</div>
						{/if}
					{/each}
				</div>
			{/if}
		</section>

		<section class="panel alert-panel" aria-labelledby="summary-title">
			<h2 id="summary-title">Alert summary</h2>
			<p>
				{protocol()?.name ?? 'This protocol'} has {formatCount(sourceCount(protocol()))} watched
				sources across program changes, multisig approvals, admin actions, and configured alert rules.
			</p>

			<div class="plain-list">
				<span>
					<strong>{coverageLabel(protocol())}</strong>
					Current coverage
				</span>
				<span>
					<strong>{data.protocolAlertCounts.critical > 0 ? 'Review' : 'Normal'}</strong>
					Critical status
				</span>
				<span>
					<strong>{latestEvent() ? displayText(latestEvent()!.summary) : 'No recent alert'}</strong>
					Latest alert
				</span>
			</div>
		</section>
	</div>

	<SimpleEventFeed
		events={allEvents()}
		protocols={feedProtocols()}
		title="Recent alerts"
		description={`Showing ${formatCount(allEvents().length)} of ${formatCount(data.protocolEventTotalCount)} indexed alerts.`}
		feedProtocolSlug={protocol()?.slug ?? null}
		emptyLabel="No recent alerts for this protocol yet."
		fallbackProtocolSlug={protocol()?.slug ?? null}
		fallbackProtocolLabel={protocol()?.name ?? null}
		explorerCluster={data.explorerCluster}
	/>

	{#if currentNextOffset() !== null}
		<div class="load-row">
			<button class="secondary-action" type="button" onclick={loadMore} disabled={loadingMore}>
				{loadingMore ? 'Loading...' : 'Load more'}
			</button>
			{#if loadMoreError}
				<p class="flash error">{loadMoreError}</p>
			{/if}
		</div>
	{/if}
</div>

<style>
	.protocol-shell {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem 0 2.4rem;
		display: grid;
		gap: 0.86rem;
	}

	.protocol-top,
	.panel {
		border: 1px solid var(--fw-border);
		border-radius: 8px;
		background: #fff;
	}

	.protocol-top {
		display: grid;
		gap: 0.86rem;
		padding: 1rem;
	}

	.back-link {
		display: inline-flex;
		align-items: center;
		justify-self: start;
		gap: 0.34rem;
		min-height: 1.8rem;
		color: #4f5c68;
		font-size: 0.82rem;
		font-weight: 760;
		line-height: 1;
		text-decoration: none;
	}

	.back-link svg {
		width: 1rem;
		height: 1rem;
		fill: none;
		stroke: currentColor;
		stroke-linecap: round;
		stroke-linejoin: round;
		stroke-width: 2;
	}

	.back-link:hover span {
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.identity-line {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.62rem;
		color: var(--fw-muted);
		font-size: 0.82rem;
		font-weight: 700;
	}

	.title-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 1rem;
		align-items: start;
	}

	.title-copy {
		display: grid;
		gap: 0.36rem;
		min-width: 0;
	}

	h1,
	h2,
	p {
		margin: 0;
	}

	h1 {
		font-size: clamp(2rem, 4vw, 3.5rem);
		line-height: 0.98;
		letter-spacing: 0;
		overflow-wrap: anywhere;
	}

	h2 {
		font-size: 1rem;
		line-height: 1.25;
	}

	p {
		color: var(--fw-muted);
		line-height: 1.5;
	}

	.page-actions,
	.load-row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.48rem;
		align-items: center;
		justify-content: flex-end;
	}

	.primary-action,
	.secondary-action {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 2.12rem;
		padding: 0 0.72rem;
		border-radius: 7px;
		font: inherit;
		font-size: 0.82rem;
		font-weight: 750;
		text-decoration: none;
		cursor: pointer;
	}

	.primary-action {
		border: 1px solid var(--fw-accent);
		background: var(--fw-accent);
		color: #fff;
	}

	.secondary-action {
		border: 1px solid rgba(23, 32, 42, 0.12);
		background: #fff;
		color: #313b44;
	}

	.secondary-action:disabled {
		cursor: wait;
		opacity: 0.62;
	}

	.primary-action:hover,
	.secondary-action:hover {
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.summary-grid {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: 0.54rem;
	}

	.summary-card {
		display: grid;
		gap: 0.1rem;
		min-width: 0;
		padding: 0.72rem;
		border: 1px solid rgba(23, 32, 42, 0.09);
		border-radius: 7px;
		background: var(--fw-panel-soft);
	}

	.summary-card strong {
		font-size: 1.12rem;
		line-height: 1;
		overflow-wrap: anywhere;
	}

	.summary-card span {
		color: var(--fw-muted);
		font-size: 0.76rem;
		font-weight: 720;
	}

	.detail-grid {
		display: grid;
		grid-template-columns: 1fr;
		gap: 0.86rem;
		align-items: stretch;
	}

	.panel {
		display: grid;
		gap: 0.78rem;
		padding: 1rem;
	}

	.panel-head {
		display: flex;
		align-items: start;
		justify-content: space-between;
		gap: 0.8rem;
	}

	.panel-head > div {
		display: grid;
		gap: 0.16rem;
	}

	.monitoring-list {
		display: grid;
		border-top: 1px solid rgba(23, 32, 42, 0.08);
	}

	.monitoring-row {
		display: grid;
		grid-template-columns: 5.6rem minmax(0, 1fr) minmax(8rem, 0.34fr);
		gap: 0.72rem;
		align-items: start;
		padding: 0.72rem 0;
		border-bottom: 1px solid rgba(23, 32, 42, 0.08);
		color: inherit;
		text-decoration: none;
	}

	a.monitoring-row:hover .monitoring-copy strong {
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.monitoring-kind,
	.monitoring-note {
		color: var(--fw-muted);
		font-size: 0.78rem;
		font-weight: 720;
		line-height: 1.35;
	}

	.monitoring-copy {
		display: grid;
		gap: 0.16rem;
		min-width: 0;
	}

	.monitoring-copy strong {
		color: var(--fw-ink);
		font-size: 0.9rem;
		line-height: 1.32;
		overflow-wrap: anywhere;
	}

	.monitoring-copy span,
	.monitoring-copy code {
		color: var(--fw-muted);
		font-size: 0.8rem;
		line-height: 1.38;
		overflow-wrap: anywhere;
	}

	.monitoring-copy code {
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
	}

	.plain-list {
		display: grid;
		border-top: 1px solid rgba(23, 32, 42, 0.08);
	}

	.plain-list span {
		display: grid;
		gap: 0.12rem;
		padding: 0.7rem 0;
		border-bottom: 1px solid rgba(23, 32, 42, 0.08);
		color: var(--fw-muted);
		font-size: 0.82rem;
		line-height: 1.4;
	}

	.plain-list strong {
		color: var(--fw-ink);
		font-size: 0.9rem;
		overflow-wrap: anywhere;
	}

	.flash {
		padding: 0.66rem 0.72rem;
		border-radius: 7px;
		font-size: 0.86rem;
		font-weight: 650;
	}

	.flash.error {
		background: rgba(156, 57, 41, 0.12);
		color: #9c3929;
	}

	.load-row {
		justify-content: flex-start;
	}

	@media (max-width: 900px) {
		.title-row,
		.detail-grid {
			grid-template-columns: 1fr;
		}

		.page-actions {
			justify-content: flex-start;
		}

		.summary-grid {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}

		.monitoring-row {
			grid-template-columns: 5.4rem minmax(0, 1fr);
		}

		.monitoring-note {
			grid-column: 2;
		}
	}

	@media (max-width: 560px) {
		.protocol-shell {
			width: min(100vw - 1rem, var(--fw-page-max));
			padding-top: 0.8rem;
		}

		.summary-grid {
			grid-template-columns: 1fr;
		}

		h1 {
			font-size: 2rem;
		}

		.monitoring-row {
			grid-template-columns: 1fr;
			gap: 0.2rem;
		}

		.monitoring-note {
			grid-column: auto;
		}
	}
</style>
