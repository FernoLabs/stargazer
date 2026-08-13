<script lang="ts">
	import { resolve } from '$app/paths';
	import ProtocolLogo from '$lib/components/dashboard/ProtocolLogo.svelte';
	import UnattributedContractIndex from '$lib/components/dashboard/UnattributedContractIndex.svelte';
	import { formatDatasetPeriod, formatExactCount } from '$lib/dashboard/format';
	import { protocolDisplayName } from '$lib/dashboard/protocols';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	let datasetPeriod = $derived(
		data.kind === 'ready' && data.catalog
			? formatDatasetPeriod(data.catalog.source.indexedFrom, data.catalog.source.asOf)
			: null
	);

	let totals = $derived.by(() => {
		if (data.kind !== 'ready' || !data.catalog) return null;
		return data.catalog.protocols.reduce(
			(result, protocol) => ({
				contracts: result.contracts + protocol.contractCount,
				versions: result.versions + protocol.versionObservationCount,
				upgrades: result.upgrades + protocol.upgradeCount
			}),
			{ contracts: 0, versions: 0, upgrades: 0 }
		);
	});
</script>

<svelte:head>
	<title>Protocols · Stargazer Stellar demo</title>
	<meta
		name="description"
		content="Browse identified Stellar protocols and every unattributed contract discovered by Stargazer."
	/>
</svelte:head>

<header class="page-heading">
	<div>
		<h1>Protocols</h1>
		<p>
			A source-backed directory of known Stellar dependencies. Contracts without verified attribution
			remain visible under their on-chain address.
		</p>
	</div>
	<a href={resolve('/app/alerts')}>View protocol alerts →</a>
</header>

<nav class="directory-filter" aria-label="Protocol attribution" data-sveltekit-preload-data="tap">
	<a
		href={resolve('/app/protocols')}
		aria-current={data.attribution === 'verified' ? 'page' : undefined}
	>
		Identified protocols
	</a>
	<a
		href={resolve('/app/protocols?attribution=unattributed')}
		aria-current={data.attribution === 'unattributed' ? 'page' : undefined}
	>
		Unattributed contracts
	</a>
</nav>

{#if datasetPeriod}
	<p class="archive-period">
		<strong>Archive window</strong> {datasetPeriod.start} through {datasetPeriod.end} ·
		{datasetPeriod.duration}
	</p>
{/if}

{#if data.kind === 'unavailable'}
	<section class="unavailable" role="status">
		<strong>Protocol index unavailable</strong>
		<p>{data.message}</p>
		<a href={resolve('/app/protocols')}>Retry</a>
	</section>
{:else if data.attribution === 'unattributed' && data.unattributedContracts}
	<UnattributedContractIndex
		index={data.unattributedContracts}
		cursor={data.unattributedCursor}
		filter="unattributed"
	/>
{:else if data.catalog && totals}
	<section class="catalog-metrics" aria-label="Identified protocol metrics">
		<dl>
			<div><dt>Protocols</dt><dd>{formatExactCount(data.catalog.protocols.length)}</dd></div>
			<div><dt>Root contracts</dt><dd>{formatExactCount(totals.contracts)}</dd></div>
			<div><dt>Version observations</dt><dd>{formatExactCount(totals.versions)}</dd></div>
			<div><dt>Exact upgrades</dt><dd>{formatExactCount(totals.upgrades)}</dd></div>
		</dl>
	</section>

	<section class="protocol-directory" aria-labelledby="directory-title">
		<header>
			<h2 id="directory-title">Identified dependencies</h2>
			<p>Open a protocol to inspect its registered roots and executable history.</p>
		</header>
		<ol>
			{#each data.catalog.protocols as protocol (protocol.protocolId)}
				<li>
					<a href={resolve('/app/protocols/[id]', { id: protocol.protocolId })}>
						<div class="protocol-identity">
							<ProtocolLogo protocolId={protocol.protocolId} size="medium" />
							<span>
								<strong>{protocolDisplayName(protocol.protocolId)}</strong>
								<code>{protocol.protocolId}</code>
							</span>
						</div>
						<dl>
							<div><dt>Roots</dt><dd>{formatExactCount(protocol.contractCount)}</dd></div>
							<div><dt>Versions</dt><dd>{formatExactCount(protocol.versionObservationCount)}</dd></div>
							<div><dt>Upgrades</dt><dd>{formatExactCount(protocol.upgradeCount)}</dd></div>
						</dl>
						<i aria-hidden="true">→</i>
					</a>
				</li>
			{/each}
		</ol>
	</section>
{/if}

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

	.page-heading p {
		max-width: 45rem;
		margin: 0.6rem 0 0;
		color: var(--app-muted);
		font-size: 0.92rem;
		line-height: 1.5;
	}

	.page-heading > a,
	.unavailable a {
		flex: 0 0 auto;
		color: var(--app-violet);
		font-size: 0.82rem;
		font-weight: 600;
		text-decoration: none;
	}

	.directory-filter {
		display: flex;
		border-top: 1px solid var(--app-rule-strong);
		border-bottom: 1px solid var(--app-rule-strong);
		background: var(--app-paper);
	}

	.directory-filter a {
		padding: 0.75rem 1rem;
		border-right: 1px solid var(--app-rule);
		border-bottom: 2px solid transparent;
		font-size: 0.82rem;
		font-weight: 600;
		text-decoration: none;
	}

	.directory-filter a:hover,
	.directory-filter a[aria-current='page'] {
		color: var(--app-violet);
	}

	.directory-filter a[aria-current='page'] {
		border-bottom-color: var(--app-violet);
	}

	.archive-period {
		margin: 0;
		padding: 0.6rem 1rem;
		border-bottom: 1px solid var(--app-rule-strong);
		background: var(--app-paper);
		color: var(--app-muted);
		font: 0.7rem/1.5 'Fragment Mono', monospace;
	}

	.archive-period strong {
		color: var(--app-ink);
	}

	.catalog-metrics {
		border-bottom: 1px solid var(--app-rule-strong);
		background: var(--app-paper);
	}

	.catalog-metrics dl {
		display: flex;
		flex-wrap: wrap;
		gap: 0;
		margin: 0;
	}

	.catalog-metrics dl > div {
		display: flex;
		gap: 0.45rem;
		align-items: baseline;
		padding: 0.7rem 1rem;
		border-right: 1px solid var(--app-rule);
	}

	.catalog-metrics dt {
		color: var(--app-muted);
		font-size: 0.72rem;
	}

	.catalog-metrics dd {
		margin: 0;
		font: 600 0.82rem/1 'Fragment Mono', monospace;
	}

	.protocol-directory {
		margin-top: 1.75rem;
	}

	.protocol-directory > header {
		display: flex;
		justify-content: space-between;
		gap: 1.5rem;
		align-items: baseline;
		padding-bottom: 0.75rem;
		border-bottom: 1px solid var(--app-ink);
	}

	.protocol-directory h2 {
		margin: 0;
		font: 600 1.15rem/1.3 'Archivo SemiCondensed', sans-serif;
	}

	.protocol-directory header p {
		margin: 0;
		color: var(--app-muted);
		font-size: 0.78rem;
	}

	.protocol-directory ol {
		margin: 0;
		padding: 0;
		background: var(--app-paper);
		list-style: none;
	}

	.protocol-directory li {
		border-bottom: 1px solid var(--app-rule);
	}

	.protocol-directory li > a {
		display: grid;
		grid-template-columns: minmax(14rem, 1fr) auto 1.5rem;
		gap: 1.5rem;
		align-items: center;
		padding: 0.9rem 1rem;
		text-decoration: none;
	}

	.protocol-directory li > a:hover {
		background: var(--app-violet-soft);
	}

	.protocol-identity {
		display: flex;
		gap: 0.75rem;
		align-items: center;
		min-width: 0;
	}

	.protocol-identity span,
	.protocol-identity strong,
	.protocol-identity code {
		display: block;
	}

	.protocol-identity strong {
		font-size: 0.9rem;
	}

	.protocol-identity code {
		margin-top: 0.2rem;
		color: var(--app-muted);
		font-size: 0.66rem;
	}

	.protocol-directory li dl {
		display: grid;
		grid-template-columns: repeat(3, 5rem);
		margin: 0;
	}

	.protocol-directory li dl > div {
		padding: 0 0.75rem;
		border-left: 1px solid var(--app-rule);
	}

	.protocol-directory dt {
		color: var(--app-muted);
		font-size: 0.65rem;
	}

	.protocol-directory dd {
		margin: 0.15rem 0 0;
		font: 600 0.78rem/1.2 'Fragment Mono', monospace;
	}

	.protocol-directory i {
		color: var(--app-violet);
		font-style: normal;
	}

	.unavailable {
		margin-top: 1.5rem;
		padding: 1rem;
		border-left: 2px solid var(--app-danger);
		background: var(--app-paper);
	}

	.unavailable p {
		margin: 0.35rem 0;
		color: var(--app-muted);
		font-size: 0.82rem;
	}

	@media (max-width: 680px) {
		.page-heading,
		.protocol-directory > header {
			align-items: flex-start;
			flex-direction: column;
		}

		.directory-filter {
			overflow-x: auto;
		}

		.directory-filter a {
			white-space: nowrap;
		}

		.protocol-directory li > a {
			grid-template-columns: minmax(0, 1fr) 1rem;
			gap: 0.75rem;
		}

		.protocol-directory li dl {
			grid-column: 1 / -1;
			grid-row: 2;
			grid-template-columns: repeat(3, 1fr);
		}

		.protocol-directory i {
			grid-column: 2;
			grid-row: 1;
		}
	}
</style>
