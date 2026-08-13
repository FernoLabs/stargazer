<script lang="ts">
	import ProtocolLogo from '$lib/components/dashboard/ProtocolLogo.svelte';
	import { formatAddress, formatExactCount } from '$lib/dashboard/format';
	import type { AlertSeverity, WalletAlert } from '$lib/dashboard/types';

	interface Props {
		alerts: readonly WalletAlert[];
	}

	let { alerts }: Props = $props();

	const dateFormatter = new Intl.DateTimeFormat('en-GB', {
		day: '2-digit',
		month: 'short',
		year: 'numeric',
		hour: '2-digit',
		minute: '2-digit',
		hourCycle: 'h23',
		timeZone: 'UTC'
	});

	function formatDate(value: string): string {
		return `${dateFormatter.format(new Date(value))} UTC`;
	}

	function severityLabel(severity: AlertSeverity): string {
		return severity === 'info' ? 'Informational' : severity[0].toUpperCase() + severity.slice(1);
	}

	function matchTypeLabel(matchType: 'contract-relationship' | 'verified-incident-participant') {
		return matchType === 'contract-relationship'
			? 'Contract relationship'
			: 'Verified incident participant';
	}
</script>

{#if alerts.length === 0}
	<div class="empty-alerts">
		<strong>No historical security findings</strong>
		<p>No match was found under the configured historical rules.</p>
	</div>
{:else}
	<ol class="timeline">
		{#each alerts as alert, index (alert.id)}
			<li class:high={alert.severity === 'high' || alert.severity === 'critical'}>
				<div class="timeline-index">
					<span>{String(index + 1).padStart(2, '0')}</span>
					<i aria-hidden="true"></i>
				</div>
				<article>
					<header>
						<div>
							<span class="severity severity-{alert.severity}">{severityLabel(alert.severity)}</span>
							{#if alert.origin === 'historical-replay'}
								<span class="origin">Historical replay</span>
							{/if}
							{#if alert.protocolName}
								<span class="protocol">
									{#if alert.protocolId}<ProtocolLogo protocolId={alert.protocolId} size="small" />{/if}
									<span>{alert.protocolName}</span>
								</span>
							{/if}
						</div>
						{#if alert.origin === 'historical-replay'}
							<div class="incident-time">
								<span>Incident time</span>
								<time datetime={alert.incidentAt}>{formatDate(alert.incidentAt)}</time>
							</div>
						{:else}
							<time datetime={alert.detectedAt}>{formatDate(alert.detectedAt)}</time>
						{/if}
					</header>

					<h3>{alert.title}</h3>
					<p>{alert.summary}</p>

					<dl>
						{#if alert.ledgerSequence}
							<div><dt>Ledger</dt><dd>{formatExactCount(alert.ledgerSequence)}</dd></div>
						{/if}
						{#if alert.contractAddress}
							<div>
								<dt>Contract</dt>
								<dd title={alert.contractAddress}>{formatAddress(alert.contractAddress, { start: 9, end: 7 })}</dd>
							</div>
						{/if}
						{#if alert.transactionHash}
							<div>
								<dt>{alert.origin === 'historical-replay' ? 'Incident transaction' : 'Transaction'}</dt>
								<dd title={alert.transactionHash}>{formatAddress(alert.transactionHash, { start: 9, end: 7 })}</dd>
							</div>
						{/if}
						{#if alert.origin === 'historical-replay'}
							<div><dt>Match</dt><dd>{matchTypeLabel(alert.matchType)}</dd></div>
						{/if}
					</dl>

					{#if alert.origin === 'historical-replay'}
						<div class="evidence">
							<span>Evidence</span>
							<ul>
								{#each alert.sources as source (source.url)}
									<li><a href={source.url} target="_blank" rel="noreferrer">{source.label}</a></li>
								{/each}
							</ul>
						</div>
						<footer class="replay-delivery">
							<strong>Historical replay · delivery not attempted</strong>
						</footer>
					{:else}
						<footer>
							<span>Delivery</span>
							<ul aria-label="Alert delivery status">
								{#each alert.deliveries as delivery (`${delivery.channel}-${delivery.sentAt ?? delivery.status}`)}
									<li class:sent={delivery.status === 'sent'}>
										{delivery.channel} · {delivery.status}
									</li>
								{/each}
							</ul>
						</footer>
					{/if}
				</article>
			</li>
		{/each}
	</ol>
{/if}

<style>
	.empty-alerts {
		padding: 2rem;
		border-top: 1px solid var(--app-ink);
		border-bottom: 1px solid var(--app-rule);
		background: var(--app-paper);
	}

	.empty-alerts p {
		margin: 0.3rem 0 0;
		color: var(--app-muted);
	}

	.timeline {
		margin: 0;
		padding: 0;
		border-top: 1px solid var(--app-ink);
		list-style: none;
	}

	.timeline > li {
		display: grid;
		grid-template-columns: 3.5rem minmax(0, 1fr);
		border-bottom: 1px solid var(--app-rule);
		background: var(--app-paper);
	}

	.timeline-index {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.7rem;
		padding-top: 1.3rem;
		border-right: 1px solid var(--app-rule);
		color: var(--app-muted);
		font: 0.64rem/1 'Fragment Mono', monospace;
	}

	.timeline-index i {
		width: 0.55rem;
		height: 0.55rem;
		background: var(--app-violet);
	}

	.high .timeline-index i {
		background: var(--app-danger);
	}

	article {
		padding: 1.2rem 1.35rem 1.35rem;
	}

	header {
		display: flex;
		justify-content: space-between;
		gap: 1rem;
		align-items: flex-start;
		margin-bottom: 0.75rem;
	}

	header > div {
		display: flex;
		gap: 0.45rem;
		align-items: center;
	}

	.severity,
	.origin,
	.protocol,
	time {
		font: 0.64rem/1.3 'Fragment Mono', monospace;
	}

	.severity,
	.origin,
	.protocol {
		padding: 0.27rem 0.42rem;
		border: 1px solid var(--app-rule);
		text-transform: uppercase;
	}

	.severity-medium,
	.severity-high,
	.severity-critical {
		border-color: #d8a8b0;
		background: #fff0f3;
		color: var(--app-danger);
	}

	.severity-info,
	.severity-low {
		border-color: #bfb0eb;
		background: var(--app-violet-soft);
		color: var(--app-violet);
	}

	.protocol {
		display: inline-flex;
		gap: 0.4rem;
		align-items: center;
		padding: 0 0.42rem 0 0;
		color: var(--app-muted);
		text-transform: none;
	}

	.origin {
		border-color: #c7b5df;
		color: var(--app-violet);
	}

	time {
		color: var(--app-muted);
		white-space: nowrap;
	}

	.incident-time {
		display: grid;
		gap: 0.18rem;
		justify-items: end;
	}

	.incident-time span {
		color: var(--app-muted);
		font: 0.58rem/1.2 'Fragment Mono', monospace;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}

	h3 {
		margin: 0 0 0.35rem;
		font: 600 1.08rem/1.25 'Archivo SemiCondensed', sans-serif;
	}

	article > p {
		max-width: 56rem;
		margin: 0;
		color: var(--app-muted);
		font-size: 0.84rem;
		line-height: 1.5;
	}

	dl {
		display: flex;
		flex-wrap: wrap;
		gap: 1rem 2.25rem;
		margin: 1rem 0 0;
		padding-top: 0.85rem;
		border-top: 1px solid var(--app-rule);
	}

	dl div {
		display: grid;
		gap: 0.15rem;
	}

	dt,
	footer > span,
	.evidence > span {
		color: var(--app-muted);
		font-size: 0.65rem;
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	dd {
		margin: 0;
		font: 0.66rem/1.4 'Fragment Mono', monospace;
	}

	footer {
		display: flex;
		gap: 1rem;
		align-items: center;
		margin-top: 1rem;
	}

	.evidence {
		display: flex;
		gap: 1rem;
		align-items: baseline;
		margin-top: 1rem;
	}

	.evidence ul {
		display: flex;
		flex-wrap: wrap;
		gap: 0.35rem 1rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.evidence a {
		color: var(--app-violet);
		font-size: 0.72rem;
		text-underline-offset: 0.18em;
	}

	.replay-delivery {
		padding-top: 0.8rem;
		border-top: 1px solid var(--app-rule);
		color: var(--app-muted);
		font: 0.65rem/1.4 'Fragment Mono', monospace;
		font-weight: 400;
	}

	footer ul {
		display: flex;
		flex-wrap: wrap;
		gap: 0.4rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	footer li {
		padding: 0.2rem 0.38rem;
		border: 1px solid var(--app-rule);
		color: var(--app-muted);
		font: 0.6rem/1.2 'Fragment Mono', monospace;
		text-transform: uppercase;
	}

	footer li.sent {
		border-color: #a9cbbd;
		color: var(--app-success);
	}

	@media (max-width: 650px) {
		.timeline > li {
			grid-template-columns: 2.4rem minmax(0, 1fr);
		}

		article {
			padding-inline: 1rem;
		}

		header {
			display: grid;
		}

		time {
			white-space: normal;
		}
	}
</style>
