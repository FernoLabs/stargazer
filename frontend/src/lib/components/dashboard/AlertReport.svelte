<script lang="ts">
	import { buildWalletAlertReport } from '$lib/dashboard/coverage';
	import type { WalletAlertReportSignal } from '$lib/dashboard/coverage';
	import type { LiveWalletResponse } from '$lib/dashboard/live';
	import type { WalletAlert } from '$lib/dashboard/types';

	interface Props {
		wallet: LiveWalletResponse;
		currentAlerts?: readonly WalletAlert[];
	}

	let { wallet, currentAlerts = [] }: Props = $props();
	let report = $derived(buildWalletAlertReport(wallet, currentAlerts));
	let signals = $derived([...report.critical, ...report.important]);

	const dateFormatter = new Intl.DateTimeFormat('en-GB', {
		day: '2-digit',
		month: 'short',
		year: 'numeric',
		timeZone: 'UTC'
	});

	function signalLabel(signal: WalletAlertReportSignal): string {
		if (signal.kind === 'historical-evidence') return 'Archive match';
		if (signal.kind === 'current-alert') return 'Recorded finding';
		if (signal.kind === 'coverage-gap') return 'Attribution gap';
		return 'Monitoring scope';
	}
</script>

<section class="alert-report" aria-labelledby="wallet-alert-report-title">
	<header>
		<div>
			<h2 id="wallet-alert-report-title">Findings and monitoring scope</h2>
			<p>Retrospective incident matches first, followed by non-critical dependency signals.</p>
		</div>
		<p class="counts">
			<strong>{report.critical.length}</strong> critical · {report.important.length} informational
		</p>
	</header>

	{#if signals.length === 0}
		<div class="empty-state">
			<strong>No configured finding matched</strong>
			<p>This is not a safety guarantee; it only reflects the checks available in this snapshot.</p>
		</div>
	{:else}
		<ol>
			{#each signals as signal (signal.id)}
				<li class:critical={signal.group === 'critical'}>
					<div class="signal-meta">
						<span>{signal.group === 'critical' ? 'Critical' : 'Info'}</span>
						<span>{signalLabel(signal)}</span>
						{#if signal.protocolName}<span>{signal.protocolName}</span>{/if}
						{#if signal.occurredAt}
							<time datetime={signal.occurredAt}>
								{dateFormatter.format(new Date(signal.occurredAt))} UTC
							</time>
						{/if}
					</div>
					<h3>{signal.title}</h3>
					<p>{signal.summary}</p>
					{#if signal.sources.length > 0}
						<div class="sources">
							{#each signal.sources as source (source.url)}
								<a href={source.url} target="_blank" rel="noreferrer">{source.label} ↗</a>
							{/each}
						</div>
					{/if}
				</li>
			{/each}
		</ol>
	{/if}

	<footer>
		<strong>Historical check:</strong> {report.historicalCheck.summary}
		<span>Live notification delivery is not connected in this demo.</span>
	</footer>
</section>

<style>
	.alert-report {
		border-top: 1px solid var(--app-ink);
		border-bottom: 1px solid var(--app-rule);
		background: var(--app-paper);
	}

	.alert-report > header {
		display: flex;
		justify-content: space-between;
		gap: 1.5rem;
		align-items: baseline;
		padding: 0.85rem 1rem;
		border-bottom: 1px solid var(--app-rule);
	}

	h2,
	h3,
	p {
		margin: 0;
	}

	h2 {
		font: 600 1.15rem/1.25 'Archivo SemiCondensed', sans-serif;
	}

	.alert-report > header div > p {
		margin-top: 0.2rem;
		color: var(--app-muted);
		font-size: 0.78rem;
	}

	.counts {
		color: var(--app-muted);
		font: 0.7rem/1.4 'Fragment Mono', monospace;
		white-space: nowrap;
	}

	.counts strong {
		color: var(--app-danger);
	}

	ol {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	li {
		padding: 0.85rem 1rem;
		border-bottom: 1px solid var(--app-rule);
		border-left: 2px solid var(--app-violet);
	}

	li.critical {
		border-left-color: var(--app-danger);
	}

	.signal-meta {
		display: flex;
		flex-wrap: wrap;
		gap: 0.35rem 0.7rem;
		align-items: baseline;
		color: var(--app-muted);
		font: 0.65rem/1.3 'Fragment Mono', monospace;
	}

	.signal-meta span:first-child {
		color: var(--app-violet);
		font-weight: 600;
		text-transform: uppercase;
	}

	.critical .signal-meta span:first-child {
		color: var(--app-danger);
	}

	.signal-meta time {
		margin-left: auto;
	}

	h3 {
		margin-top: 0.45rem;
		font: 600 1rem/1.3 'Archivo SemiCondensed', sans-serif;
	}

	li > p,
	.empty-state p {
		max-width: 58rem;
		margin-top: 0.2rem;
		color: var(--app-muted);
		font-size: 0.8rem;
		line-height: 1.45;
	}

	.sources {
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem;
		margin-top: 0.5rem;
	}

	.sources a {
		color: var(--app-violet);
		font-size: 0.72rem;
		font-weight: 600;
		text-decoration: none;
	}

	.empty-state {
		padding: 1rem;
	}

	footer {
		display: flex;
		justify-content: space-between;
		gap: 1.5rem;
		padding: 0.7rem 1rem;
		background: #f8f9fb;
		color: var(--app-muted);
		font-size: 0.72rem;
		line-height: 1.4;
	}

	footer strong {
		color: var(--app-ink);
	}

	@media (max-width: 680px) {
		.alert-report > header,
		footer {
			align-items: flex-start;
			flex-direction: column;
			gap: 0.55rem;
		}

		.signal-meta time {
			width: 100%;
			margin-left: 0;
		}
	}
</style>
