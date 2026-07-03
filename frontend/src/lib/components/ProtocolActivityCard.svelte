<script lang="ts">
	import { resolve } from '$app/paths';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import type { DefiProtocolDescriptor } from '$lib/types';

	type MetricValue = number | string | null | undefined;
	type ProtocolMetric = {
		label: string;
		value: MetricValue;
	};

	type Props = {
		protocol: DefiProtocolDescriptor;
		metrics: ProtocolMetric[];
		sourceLabel: string;
		statusLabel?: string | null;
		statusTone?: 'active' | 'warm' | 'quiet' | 'watching' | null;
		sideLabel?: string | null;
		latestLine?: string | null;
		matchLine?: string | null;
	};

	const numberFormatter = new Intl.NumberFormat('en-US');

	let {
		protocol,
		metrics,
		sourceLabel,
		statusLabel = null,
		statusTone = null,
		sideLabel = null,
		latestLine = null,
		matchLine = null
	}: Props = $props();

	function formatMetricValue(value: MetricValue) {
		if (typeof value === 'string') return value;
		return numberFormatter.format(value ?? 0);
	}
</script>

<a class="protocol-card" href={resolve('/protocols/[slug]', { slug: protocol.slug })} aria-label={`Open ${protocol.name}`}>
	<div class="protocol-copy">
		<div class="protocol-name">
			<ProtocolIdentity slug={protocol.slug} label={protocol.name} />
		</div>
		<div class="protocol-meta">
			<span class="source-breakdown">{sourceLabel}</span>
			{#if statusLabel}
				<span class={['protocol-status', statusTone]}>{statusLabel}</span>
			{/if}
		</div>
		{#if matchLine}
			<span class="match-line">{matchLine}</span>
		{/if}
	</div>

	<div class="protocol-side">
		{#if sideLabel}
			<span class={['side-meta', statusTone]}>{sideLabel}</span>
		{/if}
		<div class="protocol-metrics" aria-label={`${protocol.name} alert counts`}>
			{#each metrics as metric (metric.label)}
				<div class="metric-cell">
					<strong>{formatMetricValue(metric.value)}</strong>
					<span>{metric.label}</span>
				</div>
			{/each}
		</div>
	</div>

	{#if latestLine}
		<p class="latest-line">{latestLine}</p>
	{/if}
</a>

<style>
	.protocol-card {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 0.54rem 0.8rem;
		padding: 0.82rem 0;
		border-bottom: 1px solid rgba(23, 32, 42, 0.08);
		color: inherit;
		text-decoration: none;
	}

	.protocol-card:last-child {
		border-bottom: 0;
	}

	.protocol-card:hover .protocol-name {
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.protocol-card:hover .metric-cell {
		border-color: rgba(173, 78, 34, 0.32);
	}

	.protocol-copy {
		display: grid;
		gap: 0.26rem;
		min-width: 0;
	}

	.protocol-name {
		min-width: 0;
	}

	.protocol-meta,
	.latest-line,
	.match-line,
	.side-meta {
		color: var(--fw-muted);
		font-size: 0.82rem;
		line-height: 1.45;
	}

	.protocol-meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.24rem 0.52rem;
		min-width: 0;
	}

	.source-breakdown {
		color: #73808c;
		font-weight: 650;
	}

	.protocol-status {
		color: #425365;
		font-weight: 760;
	}

	.protocol-status.active {
		color: #167044;
	}

	.protocol-status.warm {
		color: #72510d;
	}

	.protocol-status.quiet,
	.protocol-status.watching {
		color: #596775;
	}

	.match-line {
		color: #6a4b22;
		font-weight: 650;
	}

	.protocol-side {
		display: grid;
		gap: 0.32rem;
		justify-items: end;
		align-self: start;
	}

	.side-meta {
		font-weight: 760;
		text-align: right;
	}

	.side-meta.active {
		color: #167044;
	}

	.side-meta.warm {
		color: #72510d;
	}

	.side-meta.quiet,
	.side-meta.watching {
		color: #596775;
	}

	.protocol-metrics {
		display: grid;
		grid-template-columns: repeat(3, minmax(4.4rem, 1fr));
		gap: 0.36rem;
		align-self: start;
	}

	.metric-cell {
		display: grid;
		gap: 0.05rem;
		min-width: 4.6rem;
		padding: 0.48rem 0.56rem;
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 7px;
		background: var(--fw-panel-soft);
		color: inherit;
	}

	.metric-cell strong {
		font-size: 0.96rem;
		line-height: 1;
	}

	.metric-cell span {
		color: var(--fw-muted);
		font-size: 0.72rem;
		font-weight: 700;
	}

	.latest-line {
		grid-column: 1 / -1;
		margin: 0;
	}

	@media (max-width: 640px) {
		.protocol-card {
			grid-template-columns: 1fr;
		}

		.protocol-side {
			justify-items: stretch;
		}

		.side-meta {
			text-align: left;
		}

		.protocol-metrics {
			width: 100%;
		}
	}
</style>
