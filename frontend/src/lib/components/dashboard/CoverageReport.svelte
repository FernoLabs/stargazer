<script lang="ts">
	import { buildWalletCoverageReport } from '$lib/dashboard/coverage';
	import { formatDatasetPeriod, formatExactCount } from '$lib/dashboard/format';
	import type { LiveWalletResponse } from '$lib/dashboard/live';

	interface Props {
		wallet: LiveWalletResponse;
	}

	let { wallet }: Props = $props();
	let report = $derived(buildWalletCoverageReport(wallet));
	let datasetPeriod = $derived(formatDatasetPeriod(report.sourceIndexedFrom, report.sourceAsOf));

	function statusLabel(status: 'covered' | 'partial' | 'planned'): string {
		if (status === 'covered') return 'Covered';
		if (status === 'partial') return 'Partial';
		return 'Planned';
	}

</script>

<section class="coverage-report" aria-labelledby="wallet-coverage-title">
	<header>
		<div>
			<h2 id="wallet-coverage-title">Archive coverage</h2>
			<p>What this indexed snapshot can verify for the wallet.</p>
		</div>
		<div class="archive-boundary">
			<strong>{datasetPeriod?.range ?? 'Archive period unavailable'}</strong>
			{#if report.indexedLedgerStart !== null && report.indexedLedgerEnd !== null}
				<span>
					{datasetPeriod?.duration ?? 'Bounded window'} · ledgers {formatExactCount(report.indexedLedgerStart)}–{formatExactCount(
						report.indexedLedgerEnd
					)}
				</span>
			{:else}
				<span>Ledger boundaries unavailable</span>
			{/if}
		</div>
	</header>

	<dl class="facts">
		<div>
			<dt>Contracts</dt>
			<dd>{formatExactCount(report.facts.contractCount)}</dd>
		</div>
		<div>
			<dt>Named protocols</dt>
			<dd>{formatExactCount(report.facts.protocolCount)}</dd>
		</div>
		<div>
			<dt>Named contracts</dt>
			<dd>{formatExactCount(report.facts.attributedContractCount)}</dd>
		</div>
		<div>
			<dt>Unknown contracts</dt>
			<dd>{formatExactCount(report.facts.unattributedContractCount)}</dd>
		</div>
	</dl>

	<ul class="coverage-list">
		{#each report.items as item (item.id)}
			<li>
				<div>
					<strong>{item.label}</strong>
					<p>{item.detail}</p>
				</div>
				<span class="status status-{item.status}">{statusLabel(item.status)}</span>
			</li>
		{/each}
	</ul>

	<footer>
		Source: <strong>{report.sourceLabel}</strong>. This is a bounded archive snapshot, not
		continuous monitoring or an ecosystem-wide coverage score.
	</footer>
</section>

<style>
	.coverage-report {
		border-top: 1px solid var(--app-ink);
		border-bottom: 1px solid var(--app-rule);
		background: var(--app-paper);
	}

	header {
		display: flex;
		justify-content: space-between;
		gap: 1.5rem;
		align-items: flex-start;
		padding: 1rem 1.1rem;
		border-bottom: 1px solid var(--app-rule);
	}

	h2,
	header p {
		margin: 0;
	}

	h2 {
		font: 600 1.15rem/1.2 'Archivo SemiCondensed', sans-serif;
	}

	header p {
		margin-top: 0.2rem;
		color: var(--app-muted);
		font-size: 0.875rem;
	}

	.archive-boundary {
		display: grid;
		gap: 0.2rem;
		justify-items: end;
		color: var(--app-muted);
		font: 0.75rem/1.4 'Fragment Mono', monospace;
		text-align: right;
		white-space: nowrap;
	}

	.archive-boundary strong {
		color: var(--app-ink);
		font-weight: 600;
	}

	.facts {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		margin: 0;
		border-bottom: 1px solid var(--app-rule);
	}

	.facts div {
		padding: 0.8rem 1.1rem;
		border-right: 1px solid var(--app-rule);
	}

	.facts div:last-child {
		border-right: 0;
	}

	dt {
		color: var(--app-muted);
		font-size: 0.875rem;
	}

	dd {
		margin: 0.2rem 0 0;
		font: 600 1.15rem/1.2 'Fragment Mono', monospace;
	}

	.coverage-list {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.coverage-list li {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 1rem;
		align-items: start;
		padding: 0.8rem 1.1rem;
		border-bottom: 1px solid var(--app-rule);
	}

	.coverage-list strong {
		font-size: 0.9rem;
	}

	.coverage-list p {
		max-width: 58rem;
		margin: 0.2rem 0 0;
		color: var(--app-muted);
		font-size: 0.875rem;
		line-height: 1.4;
	}

	.status {
		min-width: 4.5rem;
		padding: 0.25rem 0.4rem;
		border: 1px solid var(--app-rule-strong);
		color: var(--app-muted);
		font: 600 0.75rem/1.2 'Fragment Mono', monospace;
		text-align: center;
	}

	.status-covered {
		border-color: #9ebfad;
		color: var(--app-success);
	}

	.status-partial {
		border-color: #cfb683;
		color: var(--app-warning);
	}

	footer {
		padding: 0.7rem 1.1rem;
		color: var(--app-muted);
		font-size: 0.875rem;
		line-height: 1.45;
	}

	footer strong {
		color: var(--app-ink);
	}

	@media (max-width: 650px) {
		header {
			align-items: flex-start;
			flex-direction: column;
			gap: 0.5rem;
		}

		.archive-boundary {
			justify-items: start;
			text-align: left;
		}

		.facts {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}

		.facts div:nth-child(2) {
			border-right: 0;
		}

		.facts div:nth-child(-n + 2) {
			border-bottom: 1px solid var(--app-rule);
		}

		.coverage-list li {
			grid-template-columns: 1fr;
			gap: 0.55rem;
		}

		.status {
			width: max-content;
		}
	}
</style>
