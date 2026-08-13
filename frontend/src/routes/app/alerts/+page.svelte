<script lang="ts">
	import { resolve } from '$app/paths';
	import ProtocolLogo from '$lib/components/dashboard/ProtocolLogo.svelte';
	import type {
		AlertRegisterCategory,
		AlertRegisterProvenance,
		AlertRegisterSeverity
	} from '$lib/dashboard/alert-register';
	import { formatAddress, formatDatasetPeriod, formatExactCount } from '$lib/dashboard/format';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	let archivePeriod = $derived(
		data.archive.available
			? formatDatasetPeriod(data.archive.source.indexedFrom, data.archive.source.asOf)
			: null
	);

	type CategoryFilter = PageData['filters']['category'];

	const dateFormatter = new Intl.DateTimeFormat('en-GB', {
		day: '2-digit',
		month: 'short',
		year: 'numeric',
		timeZone: 'UTC'
	});

	function categoryLabel(category: AlertRegisterCategory): string {
		if (category === 'incident') return 'Incident';
		if (category === 'contract-upgrade') return 'Protocol upgrade';
		return 'Controller activity';
	}

	function severityLabel(severity: AlertRegisterSeverity): string {
		if (severity === 'critical') return 'Critical';
		if (severity === 'high') return 'Review';
		return 'Change';
	}

	function provenanceLabel(provenance: AlertRegisterProvenance): string {
		if (provenance === 'verified-intelligence') return 'Independently sourced incident';
		if (provenance === 'archive-index') return 'Indexed Stellar archive';
		return 'Forward controller watcher';
	}

	function formatDate(value: string | null): string {
		if (!value) return 'Date unavailable';
		const date = new Date(value);
		return Number.isNaN(date.valueOf()) ? 'Date unavailable' : dateFormatter.format(date);
	}

	function filterCount(category: CategoryFilter): number {
		if (category === 'all') {
			return data.totals.incidents + (data.totals.upgrades ?? 0) + (data.totals.controlChanges ?? 0);
		}
		if (category === 'incident') return data.totals.incidents;
		if (category === 'contract-upgrade') return data.totals.upgrades ?? 0;
		return data.totals.controlChanges ?? 0;
	}
</script>

<svelte:head>
	<title>Alerts · Stargazer Stellar demo</title>
	<meta
		name="description"
		content="Review Stellar incidents, contract upgrades, and submitted controller activity with evidence."
	/>
</svelte:head>

<header class="page-heading">
	<div>
		<h1>Alerts</h1>
		<p>Incidents and protocol-level changes, summarized for review with technical evidence attached.</p>
	</div>
	<p class="totals">
		<strong>{formatExactCount(filterCount('all'))}</strong> records ·
		{formatExactCount(data.totals.incidents)} incidents ·
		{data.totals.upgrades === null ? '—' : formatExactCount(data.totals.upgrades)} upgrades ·
		{data.totals.controlChanges === null
			? '—'
			: formatExactCount(data.totals.controlChanges)} controller alerts
	</p>
</header>

{#if data.archive.available}
	<p class="source-line">
		<strong>Archive-backed Stellar Mainnet demo.</strong>
		{#if archivePeriod}
			Contract changes cover {archivePeriod.start} through {archivePeriod.end}
			({archivePeriod.duration});
		{:else}
			Contract changes are indexed through {formatDate(data.archive.source.asOf)};
		{/if}
		incident intelligence is sourced independently.
	</p>
{:else}
	<p class="source-line source-error" role="status">
		<strong>Archive changes unavailable.</strong> {data.archive.message} Retrospective incidents remain
		visible.
	</p>
{/if}

<nav class="filters" aria-label="Filter alerts" data-sveltekit-preload-data="tap">
	<a href={resolve('/app/alerts')} aria-current={data.filters.category === 'all' ? 'page' : undefined}>
		All <span>{formatExactCount(filterCount('all'))}</span>
	</a>
	<a
		href={resolve('/app/alerts?category=incident')}
		aria-current={data.filters.category === 'incident' ? 'page' : undefined}
	>
		Incidents <span>{formatExactCount(filterCount('incident'))}</span>
	</a>
	<a
		href={resolve('/app/alerts?category=contract-upgrade')}
		aria-current={data.filters.category === 'contract-upgrade' ? 'page' : undefined}
	>
		Upgrades <span>{formatExactCount(filterCount('contract-upgrade'))}</span>
	</a>
	{#if filterCount('control-change') > 0}
		<a
			href={resolve('/app/alerts?category=control-change')}
			aria-current={data.filters.category === 'control-change' ? 'page' : undefined}
		>
			Controllers <span>{formatExactCount(filterCount('control-change'))}</span>
		</a>
	{/if}
</nav>

<section class="feed" aria-label="Alert feed">
	{#if data.records.length === 0}
		<div class="empty-state">
			<strong>No alert matches this filter.</strong>
			<a href={resolve('/app/alerts')}>Show all alerts →</a>
		</div>
	{:else}
		<ol>
			{#each data.records as record (record.id)}
				<li class={`severity-${record.severity}`}>
					<article>
						<header class="alert-meta">
							<div>
								<strong>{severityLabel(record.severity)}</strong>
								<span>{categoryLabel(record.category)}</span>
								<span class="protocol">
									{#if record.protocolId}
										<ProtocolLogo protocolId={record.protocolId} size="small" />
									{/if}
									{record.protocols.join(' · ')}
								</span>
							</div>
							<time datetime={record.recordedAt ?? undefined}>{formatDate(record.recordedAt)}</time>
						</header>

						<h2>{record.title}</h2>
						<p>{record.summary}</p>

						<div class="alert-actions">
							{#if record.reportWallet}
								<a href={resolve('/app/wallet/[address]', { address: record.reportWallet })}>
									Check wallet exposure →
								</a>
							{/if}
							<details>
								<summary>Evidence and payload</summary>
								<div class="evidence">
									<dl>
										<div><dt>Source</dt><dd>{provenanceLabel(record.provenance)}</dd></div>
										{#if record.ledger !== null}
											<div><dt>Ledger</dt><dd>{formatExactCount(record.ledger)}</dd></div>
										{/if}
										{#if record.context}<div><dt>Payload / change</dt><dd><code>{record.context}</code></dd></div>{/if}
									</dl>

									{#if record.category === 'control-change'}
										<p class="multisig-boundary">
											Controller payloads are visible after successful on-chain submission. Unsigned or
											partially signed proposals exchanged off-chain are not visible without a coordinator
											integration.
										</p>
									{/if}

									<div class="evidence-links">
										{#if record.address}
											<a
												href={`https://stellar.expert/explorer/public/${record.address.startsWith('C') ? 'contract' : 'account'}/${record.address}`}
												target="_blank"
												rel="noreferrer"
												title={record.address}
											>
												{record.address.startsWith('C') ? 'Contract' : 'Controller'}
												{formatAddress(record.address, { start: 9, end: 7 })} ↗
											</a>
										{/if}
										{#if record.transactionHash}
											<a
												href={`https://stellar.expert/explorer/public/tx/${record.transactionHash}`}
												target="_blank"
												rel="noreferrer"
												title={record.transactionHash}
											>
												Transaction {formatAddress(record.transactionHash, { start: 9, end: 7 })} ↗
											</a>
										{/if}
										{#each record.sources as source (source.url)}
											<a href={source.url} target="_blank" rel="noreferrer">{source.label} ↗</a>
										{/each}
									</div>
								</div>
							</details>
						</div>
					</article>
				</li>
			{/each}
		</ol>
	{/if}
</section>

<style>
	.page-heading {
		display: flex;
		justify-content: space-between;
		gap: 2rem;
		align-items: end;
		padding: 0.5rem 0 1.5rem;
	}

	.page-heading h1 {
		margin: 0;
		font: 600 clamp(2rem, 4vw, 2.65rem)/1.05 'Archivo SemiCondensed', sans-serif;
		letter-spacing: -0.035em;
	}

	.page-heading > div > p {
		max-width: 43rem;
		margin: 0.6rem 0 0;
		color: var(--app-muted);
		font-size: 0.92rem;
		line-height: 1.5;
	}

	.totals {
		margin: 0;
		color: var(--app-muted);
		font: 0.7rem/1.5 'Fragment Mono', monospace;
		white-space: nowrap;
	}

	.totals strong {
		color: var(--app-ink);
		font-size: 0.82rem;
	}

	.source-line {
		margin: 0 0 1rem;
		padding: 0.65rem 0;
		border-top: 1px solid var(--app-rule);
		border-bottom: 1px solid var(--app-rule);
		color: var(--app-muted);
		font-size: 0.76rem;
		line-height: 1.4;
	}

	.source-line strong {
		color: var(--app-ink);
	}

	.source-error strong {
		color: var(--app-danger);
	}

	.filters {
		display: flex;
		overflow-x: auto;
		border-top: 1px solid var(--app-rule-strong);
		border-bottom: 1px solid var(--app-rule-strong);
		background: var(--app-paper);
	}

	.filters a {
		display: inline-flex;
		gap: 0.45rem;
		align-items: center;
		padding: 0.7rem 1rem;
		border-right: 1px solid var(--app-rule);
		border-bottom: 2px solid transparent;
		font-size: 0.8rem;
		font-weight: 600;
		text-decoration: none;
		white-space: nowrap;
	}

	.filters a span {
		color: var(--app-muted);
		font: 0.65rem/1 'Fragment Mono', monospace;
	}

	.filters a:hover,
	.filters a[aria-current='page'] {
		color: var(--app-violet);
	}

	.filters a[aria-current='page'] {
		border-bottom-color: var(--app-violet);
	}

	.feed {
		border-bottom: 1px solid var(--app-rule-strong);
		background: var(--app-paper);
	}

	.feed ol {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.feed li {
		border-bottom: 1px solid var(--app-rule);
		border-left: 3px solid var(--app-violet);
	}

	.feed li:last-child {
		border-bottom: 0;
	}

	.feed li.severity-critical {
		border-left-color: var(--app-danger);
	}

	.feed li.severity-high {
		border-left-color: var(--app-warning);
	}

	.feed article {
		padding: 0.9rem 1rem;
	}

	.alert-meta {
		display: flex;
		justify-content: space-between;
		gap: 1rem;
		align-items: center;
		color: var(--app-muted);
		font: 0.66rem/1.3 'Fragment Mono', monospace;
	}

	.alert-meta > div {
		display: flex;
		flex-wrap: wrap;
		gap: 0.4rem 0.7rem;
		align-items: center;
	}

	.alert-meta > div > strong {
		color: var(--app-violet);
		text-transform: uppercase;
	}

	.severity-critical .alert-meta > div > strong {
		color: var(--app-danger);
	}

	.severity-high .alert-meta > div > strong {
		color: var(--app-warning);
	}

	.protocol {
		display: inline-flex;
		gap: 0.4rem;
		align-items: center;
	}

	.alert-meta time {
		white-space: nowrap;
	}

	.feed h2 {
		margin: 0.45rem 0 0;
		font: 600 1.08rem/1.3 'Archivo SemiCondensed', sans-serif;
	}

	.feed article > p {
		max-width: 60rem;
		margin: 0.2rem 0 0;
		color: var(--app-muted);
		font-size: 0.8rem;
		line-height: 1.45;
	}

	.alert-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.6rem 1.2rem;
		align-items: flex-start;
		margin-top: 0.55rem;
	}

	.alert-actions > a,
	.alert-actions summary,
	.evidence-links a {
		color: var(--app-violet);
		font-size: 0.74rem;
		font-weight: 600;
		text-decoration: none;
	}

	.alert-actions details {
		min-width: min(100%, 52rem);
	}

	.alert-actions summary {
		cursor: pointer;
		list-style: none;
	}

	.alert-actions summary::-webkit-details-marker {
		display: none;
	}

	.alert-actions summary::after {
		margin-left: 0.35rem;
		content: '+';
	}

	.alert-actions details[open] summary::after {
		content: '−';
	}

	.evidence {
		margin-top: 0.65rem;
		padding: 0.85rem 1rem;
		border-left: 2px solid var(--app-rule-strong);
		background: #f7f8fb;
	}

	.evidence dl {
		display: grid;
		gap: 0.55rem;
		margin: 0;
	}

	.evidence dl > div {
		display: grid;
		grid-template-columns: 7rem minmax(0, 1fr);
		gap: 0.75rem;
	}

	.evidence dt {
		color: var(--app-muted);
		font-size: 0.68rem;
	}

	.evidence dd {
		min-width: 0;
		margin: 0;
		font-size: 0.74rem;
	}

	.evidence code {
		overflow-wrap: anywhere;
		white-space: pre-wrap;
	}

	.multisig-boundary {
		max-width: 48rem;
		margin: 0.7rem 0 0;
		color: var(--app-muted);
		font-size: 0.7rem;
		line-height: 1.45;
	}

	.evidence-links {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem 1rem;
		margin-top: 0.75rem;
	}

	.empty-state {
		display: flex;
		justify-content: space-between;
		gap: 1rem;
		padding: 1.25rem 1rem;
	}

	.empty-state a {
		color: var(--app-violet);
		font-size: 0.8rem;
		font-weight: 600;
	}

	@media (max-width: 720px) {
		.page-heading {
			align-items: flex-start;
			flex-direction: column;
		}

		.totals {
			white-space: normal;
		}

		.alert-meta {
			align-items: flex-start;
			flex-direction: column;
			gap: 0.4rem;
		}

		.evidence dl > div {
			grid-template-columns: 1fr;
			gap: 0.15rem;
		}
	}
</style>
