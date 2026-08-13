<script lang="ts">
	import { resolve } from '$app/paths';
	import ProtocolLogo from '$lib/components/dashboard/ProtocolLogo.svelte';
	import { formatAddress, formatDatasetPeriod, formatExactCount } from '$lib/dashboard/format';
	import type { LiveContractExecutableVersion } from '$lib/dashboard/live';
	import { protocolDisplayName } from '$lib/dashboard/protocols';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	let datasetPeriod = $derived(
		data.kind === 'ready' ? formatDatasetPeriod(data.source.indexedFrom, data.source.asOf) : null
	);

	const dateFormatter = new Intl.DateTimeFormat('en-GB', {
		day: '2-digit',
		month: 'short',
		year: 'numeric',
		hour: '2-digit',
		minute: '2-digit',
		hourCycle: 'h23',
		timeZone: 'UTC'
	});

	function formatDate(value: string | null): string {
		if (!value) return 'Date unavailable';
		const date = new Date(value);
		return Number.isNaN(date.valueOf()) ? 'Date unavailable' : `${dateFormatter.format(date)} UTC`;
	}

	function shortHash(value: string | null): string {
		return value ? `${value.slice(0, 10)}…${value.slice(-8)}` : 'No Wasm hash';
	}

	function versionLabel(version: LiveContractExecutableVersion): string {
		if (version.exactUpgrade) return 'Exact upgrade';
		if (version.sourceKind === 'created') return 'Deployment';
		if (version.activeBeforeWindow) return 'Pre-window baseline';
		return version.sourceKind.replaceAll('_', ' ');
	}
</script>

<svelte:head>
	<title>
		{data.kind === 'ready' ? protocolDisplayName(data.protocol.protocolId) : data.protocolId} ·
		Stargazer
	</title>
</svelte:head>

<a class="back-link" href={resolve('/app/protocols')}>← All protocols</a>

{#if data.kind === 'unavailable'}
	<section class="message-state" role="status">
		<h1>Protocol history unavailable</h1>
		<p>{data.message}</p>
		<a href={resolve('/app/protocols/[id]', { id: data.protocolId })}>Retry</a>
	</section>
{:else if data.kind === 'missing'}
	<section class="message-state">
		<h1>Protocol not found</h1>
		<p>No identified protocol uses ID <code>{data.protocolId}</code>.</p>
		<a href={resolve('/app/protocols?attribution=unattributed')}>Browse unattributed contracts</a>
	</section>
{:else}
	<header class="protocol-header">
		<div class="protocol-title">
			<ProtocolLogo protocolId={data.protocol.protocolId} size="large" />
			<div>
				<h1>{protocolDisplayName(data.protocol.protocolId)}</h1>
				<code>{data.protocol.protocolId}</code>
			</div>
		</div>
		<a href={resolve('/app/alerts?category=contract-upgrade')}>View upgrade alerts →</a>
	</header>

	<section class="protocol-metrics" aria-label="Protocol index metrics">
		<dl>
			<div><dt>Root contracts</dt><dd>{formatExactCount(data.protocol.contractCount)}</dd></div>
			<div><dt>Version observations</dt><dd>{formatExactCount(data.protocol.versionObservationCount)}</dd></div>
			<div><dt>Exact upgrades</dt><dd>{formatExactCount(data.protocol.upgradeCount)}</dd></div>
			<div><dt>Controllers monitored</dt><dd>{formatExactCount(data.controllers.length)}</dd></div>
		</dl>
	</section>

	{#if data.controllers.length > 0}
		<section class="controllers" aria-labelledby="controllers-title">
			<header>
				<h2 id="controllers-title">Controller monitoring</h2>
				<p>Current Classic account policy from the forward watcher.</p>
			</header>
			<ul>
				{#each data.controllers as controller (controller.accountId)}
					<li>
						<div>
							<strong>{controller.role}</strong>
							<a
								href={`https://stellar.expert/explorer/public/account/${controller.accountId}`}
								target="_blank"
								rel="noreferrer"
							>{formatAddress(controller.accountId, { start: 10, end: 8 })} ↗</a>
						</div>
						<dl>
							<div><dt>Policy</dt><dd>{controller.policy.classification.replaceAll('-', ' ')}</dd></div>
							<div>
								<dt>Thresholds</dt>
								<dd>{controller.policy.thresholds.low}/{controller.policy.thresholds.medium}/{controller.policy.thresholds.high}</dd>
							</div>
							<div>
								<dt>Active signers</dt>
								<dd>{controller.policy.signers.filter((signer) => signer.weight > 0).length}</dd>
							</div>
						</dl>
					</li>
				{/each}
			</ul>
		</section>
	{/if}

	<section class="contracts" aria-labelledby="contracts-title">
		<header>
			<h2 id="contracts-title">Contract executable history</h2>
			<p>Registered roots only. Every exact transition links back to its Stellar transaction.</p>
		</header>

		<div class="contract-list">
			{#each data.protocol.contracts as contract (contract.contractAddress)}
				<article>
					<header>
						<div>
							<strong>{contract.roles.map((role) => role.role).join(', ')}</strong>
							<a
								href={`https://stellar.expert/explorer/public/contract/${contract.contractAddress}`}
								target="_blank"
								rel="noreferrer"
								title={contract.contractAddress}
							>{formatAddress(contract.contractAddress, { start: 11, end: 9 })} ↗</a>
						</div>
						<dl>
							<div>
								<dt>Transactions</dt>
								<dd>{contract.usage ? formatExactCount(contract.usage.transactionCount) : '—'}</dd>
							</div>
							<div><dt>Versions</dt><dd>{formatExactCount(contract.versions.length)}</dd></div>
							<div>
								<dt>Upgrades</dt>
								<dd>{formatExactCount(contract.versions.filter((version) => version.exactUpgrade).length)}</dd>
							</div>
						</dl>
					</header>

					<details>
						<summary>View versions and evidence</summary>
						{#if contract.versions.length === 0}
							<p class="empty-version">No executable observation in this archive window.</p>
						{:else}
							<ol>
								{#each contract.versions as version (`${version.ledger}:${version.applyOrder}`)}
									<li class:upgrade={version.exactUpgrade}>
										<div>
											<strong>{versionLabel(version)}</strong>
											<time datetime={version.observedAt ?? undefined}>{formatDate(version.observedAt)}</time>
										</div>
										<div>
											<code>{shortHash(version.wasmHash)}</code>
											<span>ledger {formatExactCount(version.ledger)}</span>
										</div>
										<a
											href={`https://stellar.expert/explorer/public/tx/${version.transactionHash}`}
											target="_blank"
											rel="noreferrer"
										>Transaction ↗</a>
									</li>
								{/each}
							</ol>
						{/if}
					</details>
				</article>
			{/each}
		</div>
	</section>

	<footer class="source-note">
		<strong>Data scope</strong>
		<span>
			{data.source.label}.
			{#if datasetPeriod}
				Coverage: {datasetPeriod.start} through {datasetPeriod.end} ({datasetPeriod.duration}).
			{/if}
			{data.source.disclaimer}
		</span>
	</footer>
{/if}

<style>
	.back-link {
		display: inline-block;
		margin-bottom: 1.4rem;
		color: var(--app-muted);
		font-size: 0.78rem;
		font-weight: 600;
		text-decoration: none;
	}

	.protocol-header {
		display: flex;
		justify-content: space-between;
		gap: 2rem;
		align-items: center;
	}

	.protocol-title {
		display: flex;
		gap: 0.9rem;
		align-items: center;
	}

	.protocol-title h1,
	.message-state h1 {
		margin: 0;
		font: 600 clamp(2rem, 4vw, 2.65rem)/1.05 'Archivo SemiCondensed', sans-serif;
		letter-spacing: -0.035em;
	}

	.protocol-title code {
		display: block;
		margin-top: 0.25rem;
		color: var(--app-muted);
		font-size: 0.7rem;
	}

	.protocol-header > a,
	.message-state a,
	.contracts a,
	.controllers a {
		color: var(--app-violet);
		font-size: 0.78rem;
		font-weight: 600;
		text-decoration: none;
	}

	.protocol-metrics {
		margin-top: 1.5rem;
		border-top: 1px solid var(--app-rule-strong);
		border-bottom: 1px solid var(--app-rule-strong);
		background: var(--app-paper);
	}

	.protocol-metrics dl {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		margin: 0;
	}

	.protocol-metrics dl > div {
		padding: 0.85rem 1rem;
		border-right: 1px solid var(--app-rule);
	}

	.protocol-metrics dl > div:last-child {
		border-right: 0;
	}

	.protocol-metrics dt,
	.controllers dt,
	.contracts dt {
		color: var(--app-muted);
		font-size: 0.68rem;
	}

	.protocol-metrics dd {
		margin: 0.25rem 0 0;
		font: 600 0.95rem/1.2 'Fragment Mono', monospace;
	}

	.controllers,
	.contracts {
		margin-top: 2rem;
	}

	.controllers > header,
	.contracts > header {
		display: flex;
		justify-content: space-between;
		gap: 1.5rem;
		align-items: baseline;
		padding-bottom: 0.75rem;
		border-bottom: 1px solid var(--app-ink);
	}

	.controllers h2,
	.contracts h2 {
		margin: 0;
		font: 600 1.15rem/1.3 'Archivo SemiCondensed', sans-serif;
	}

	.controllers header p,
	.contracts header p {
		margin: 0;
		color: var(--app-muted);
		font-size: 0.78rem;
	}

	.controllers ul,
	.contract-list {
		margin: 0;
		padding: 0;
		background: var(--app-paper);
		list-style: none;
	}

	.controllers li,
	.contract-list article > header {
		display: grid;
		grid-template-columns: minmax(14rem, 1fr) auto;
		gap: 1.5rem;
		align-items: center;
		padding: 0.9rem 1rem;
		border-bottom: 1px solid var(--app-rule);
	}

	.controllers li > div > strong,
	.controllers li > div > a,
	.contract-list article > header > div > strong,
	.contract-list article > header > div > a {
		display: block;
	}

	.controllers li > div > a,
	.contract-list article > header > div > a {
		margin-top: 0.25rem;
		font-family: 'Fragment Mono', monospace;
	}

	.controllers li > dl,
	.contract-list article > header > dl {
		display: grid;
		grid-template-columns: repeat(3, minmax(5rem, auto));
		margin: 0;
	}

	.controllers li > dl > div,
	.contract-list article > header > dl > div {
		padding: 0 0.75rem;
		border-left: 1px solid var(--app-rule);
	}

	.controllers dd,
	.contracts dd {
		margin: 0.15rem 0 0;
		font: 600 0.74rem/1.25 'Fragment Mono', monospace;
	}

	.contract-list article {
		border-bottom: 1px solid var(--app-rule-strong);
	}

	.contract-list article > header {
		border-bottom: 0;
	}

	details {
		border-top: 1px solid var(--app-rule);
	}

	summary {
		padding: 0.7rem 1rem;
		color: var(--app-violet);
		cursor: pointer;
		font-size: 0.76rem;
		font-weight: 600;
	}

	details ol {
		margin: 0;
		padding: 0;
		border-top: 1px solid var(--app-rule);
		background: #f8f9fb;
		list-style: none;
	}

	details li {
		display: grid;
		grid-template-columns: minmax(12rem, 1fr) minmax(12rem, 1fr) auto;
		gap: 1rem;
		align-items: center;
		padding: 0.75rem 1rem;
		border-bottom: 1px solid var(--app-rule);
		font-size: 0.75rem;
	}

	details li:last-child {
		border-bottom: 0;
	}

	details li.upgrade {
		border-left: 2px solid var(--app-warning);
	}

	details li strong,
	details li time,
	details li code,
	details li span {
		display: block;
	}

	details li time,
	details li span {
		margin-top: 0.2rem;
		color: var(--app-muted);
		font-size: 0.68rem;
	}

	.empty-version {
		margin: 0;
		padding: 0.85rem 1rem;
		border-top: 1px solid var(--app-rule);
		color: var(--app-muted);
		font-size: 0.78rem;
	}

	.source-note {
		display: grid;
		grid-template-columns: 8rem minmax(0, 1fr);
		gap: 1rem;
		margin-top: 2rem;
		padding-top: 1rem;
		border-top: 1px solid var(--app-rule);
		color: var(--app-muted);
		font-size: 0.76rem;
		line-height: 1.45;
	}

	.source-note strong {
		color: var(--app-ink);
	}

	.message-state {
		max-width: 42rem;
		padding: 1.5rem;
		border: 1px solid var(--app-rule);
		background: var(--app-paper);
	}

	.message-state p {
		color: var(--app-muted);
	}

	@media (max-width: 720px) {
		.protocol-header,
		.controllers > header,
		.contracts > header {
			align-items: flex-start;
			flex-direction: column;
		}

		.protocol-metrics dl {
			grid-template-columns: repeat(2, 1fr);
		}

		.protocol-metrics dl > div:nth-child(2) {
			border-right: 0;
		}

		.protocol-metrics dl > div:nth-child(-n + 2) {
			border-bottom: 1px solid var(--app-rule);
		}

		.controllers li,
		.contract-list article > header {
			grid-template-columns: 1fr;
		}

		.controllers li > dl,
		.contract-list article > header > dl {
			grid-template-columns: repeat(3, 1fr);
		}

		details li {
			grid-template-columns: 1fr;
			gap: 0.55rem;
		}
	}
</style>
