<script lang="ts">
	import { resolve } from '$app/paths';
	import { formatAddress, formatDatasetPeriod, formatExactCount } from '$lib/dashboard/format';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	let metric = $derived(data.kind === 'ready' ? data.leaderboard.metric : data.metric);
	let sortLabel = $derived(
		metric === 'transactions' ? 'contract transactions' : 'protocols used'
	);
	let datasetPeriod = $derived(
		data.kind === 'ready'
			? formatDatasetPeriod(data.leaderboard.source.indexedFrom, data.leaderboard.source.asOf)
			: null
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

	function formatDate(value: string): string {
		return `${dateFormatter.format(new Date(value))} UTC`;
	}

	function successRate(successful: number, total: number): string {
		return total === 0 ? '—' : `${((successful / total) * 100).toFixed(1)}%`;
	}
</script>

<svelte:head>
	<title>Top wallet activity · Stargazer</title>
	<meta
		name="description"
		content="Explore the most active Stellar wallets by contract transactions or protocols used."
	/>
</svelte:head>

<section class="leaderboard-heading" aria-labelledby="wallet-activity-title">
	<div>
		<h1 id="wallet-activity-title">Top wallet activity</h1>
	</div>
	<p>
		Most active wallets first. Rank the archive by contract-interacting transactions or by how
		many identified protocols each wallet has used.
	</p>
</section>

<section class="leaderboard" aria-label="Top wallet activity">
	<header>
		<form
			class="sort-controls"
			method="get"
			action={resolve('/app/leaderboard')}
			aria-label="Sort top wallet activity by"
		>
			<span>Rank by</span>
			<button
				type="submit"
				name="sort"
				value="transactions"
				aria-pressed={metric === 'transactions'}
			>
				Contract transactions
			</button>
			<button
				type="submit"
				name="sort"
				value="protocols"
				aria-pressed={metric === 'protocols'}
			>
				Protocols used
			</button>
		</form>
		<div class="snapshot">
			{#if data.kind === 'ready'}
				<span>
					{data.leaderboard.rows.length} wallets · sorted by {sortLabel} ·
					{datasetPeriod?.duration ?? 'bounded window'}
				</span>
				<strong>{datasetPeriod?.range ?? data.leaderboard.source.label}</strong>
			{:else if data.kind === 'building'}
				<span>Exact rows withheld</span>
				<strong>Archive-window snapshot building</strong>
			{:else}
				<span>No rows available</span>
				<strong>Read service unavailable</strong>
			{/if}
		</div>
	</header>

	{#if data.kind === 'ready'}
		<div class="table-scroll">
			<table>
				<thead>
					<tr>
						<th scope="col">Rank</th>
						<th scope="col">Wallet</th>
						<th scope="col" class:active-column={metric === 'transactions'}>
							Contract transactions
						</th>
						<th scope="col" class:active-column={metric === 'protocols'}>Protocols used</th>
						<th scope="col">Contracts</th>
						<th scope="col">Success rate</th>
						<th scope="col">Last active</th>
						<th scope="col" aria-label="Inspect"></th>
					</tr>
				</thead>
				<tbody>
					{#each data.leaderboard.rows as row (row.accountAddress)}
						<tr>
							<td class="rank">{String(row.rank).padStart(2, '0')}</td>
							<th scope="row">
								<a
									href={resolve('/app/wallet/[address]', { address: row.accountAddress })}
									title={row.accountAddress}
								>
									{formatAddress(row.accountAddress, { start: 10, end: 8 })}
								</a>
							</th>
							<td class:active-value={metric === 'transactions'}>
								{formatExactCount(row.transactionCount)}
							</td>
							<td class:active-value={metric === 'protocols'}>{row.protocolCount}</td>
							<td>{row.contractCount}</td>
							<td>{successRate(row.successfulTransactionCount, row.transactionCount)}</td>
							<td><time datetime={row.lastInteractionAt}>{formatDate(row.lastInteractionAt)}</time></td>
							<td>
								<a
									class="inspect-link"
									href={resolve('/app/wallet/[address]', { address: row.accountAddress })}
									aria-label={`Inspect ${formatAddress(row.accountAddress)}`}
								>→</a>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>

		<footer>
			<p>{data.leaderboard.source.disclaimer}</p>
			<p>
				These rows come from the atomically published wallet activity snapshot{datasetPeriod
					? ` covering ${datasetPeriod.start} through ${datasetPeriod.end} (${datasetPeriod.duration})`
					: ''}.
			</p>
		</footer>
	{:else}
		<div class="leaderboard-state" role="status">
			{#if data.kind === 'building'}
				<strong>Top wallet activity is being computed.</strong>
				<p>{data.message}</p>
				<p>
					No partial or synthetic ranking is shown. Refresh this page after the serving snapshot is
					atomically published.
				</p>
			{:else}
				<strong>Top wallet activity could not be loaded.</strong>
				<p>{data.message}</p>
				<p>No cached preview rows have been substituted.</p>
			{/if}
			<form method="get" action={resolve('/app/leaderboard')}>
				<input type="hidden" name="sort" value={metric} />
				<button type="submit">Retry the live query</button>
			</form>
		</div>
	{/if}
</section>

<style>
	.leaderboard-heading {
		display: flex;
		justify-content: space-between;
		gap: 2rem;
		align-items: end;
		padding: 0.5rem 0 1.5rem;
	}

	h1 {
		margin: 0;
		font: 600 clamp(2rem, 4vw, 2.65rem)/1.05 'Archivo SemiCondensed', sans-serif;
		letter-spacing: -0.035em;
	}

	.leaderboard-heading > p {
		max-width: 42rem;
		margin: 0;
		color: var(--app-muted);
		font-size: 0.92rem;
		line-height: 1.55;
	}

	.leaderboard {
		border-top: 2px solid var(--app-ink);
		background: var(--app-paper);
	}

	.leaderboard > header {
		display: flex;
		justify-content: space-between;
		gap: 2rem;
		align-items: center;
		min-height: 4.2rem;
		padding: 0.75rem 1rem;
		border-bottom: 1px solid var(--app-rule);
	}

	.sort-controls {
		display: flex;
		gap: 0.45rem;
		align-items: center;
	}

	.sort-controls > span {
		margin-right: 0.4rem;
		color: var(--app-muted);
		font-size: 0.68rem;
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.sort-controls button {
		padding: 0.48rem 0.68rem;
		border: 1px solid var(--app-rule);
		border-radius: 4px;
		background: transparent;
		color: var(--app-muted);
		cursor: pointer;
		font-family: 'Source Sans 3', sans-serif;
		font-size: 0.72rem;
		font-weight: 600;
	}

	.sort-controls button:hover {
		border-color: var(--app-rule-strong);
		color: var(--app-ink);
	}

	.sort-controls button[aria-pressed='true'] {
		border-color: var(--app-ink);
		background: var(--app-ink);
		color: white;
	}

	.snapshot {
		display: grid;
		gap: 0.15rem;
		text-align: right;
	}

	.snapshot span,
	.snapshot strong {
		font: 0.64rem/1.3 'Fragment Mono', monospace;
	}

	.snapshot span {
		color: var(--app-muted);
	}

	.snapshot strong {
		color: var(--app-violet);
	}

	.table-scroll {
		overflow-x: auto;
		-webkit-overflow-scrolling: touch;
	}

	table {
		width: 100%;
		min-width: 960px;
		border-collapse: collapse;
		font-size: 0.78rem;
		text-align: left;
	}

	th,
	td {
		padding: 0.9rem 0.85rem;
		border-bottom: 1px solid var(--app-rule);
		white-space: nowrap;
	}

	thead th {
		background: #f8f8f5;
		color: var(--app-muted);
		font-size: 0.65rem;
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}

	thead th.active-column {
		box-shadow: inset 0 -2px var(--app-violet);
		color: var(--app-ink);
	}

	tbody th {
		font-weight: 400;
	}

	tbody th a {
		font: 600 0.69rem/1.3 'Fragment Mono', monospace;
		text-decoration: none;
	}

	tbody th a:hover,
	.inspect-link:hover {
		color: var(--app-violet);
	}

	.rank,
	td,
	time {
		font-family: 'Fragment Mono', monospace;
	}

	.rank {
		color: var(--app-muted);
	}

	td.active-value {
		background: #faf8ff;
		color: var(--app-violet);
		font-weight: 600;
	}

	time {
		color: var(--app-muted);
		font-size: 0.66rem;
	}

	.inspect-link {
		display: inline-block;
		padding: 0.2rem;
		font-size: 1rem;
		text-decoration: none;
	}

	.leaderboard-state {
		padding: 2rem 1.25rem 2.4rem;
		border-bottom: 1px solid var(--app-rule);
	}

	.leaderboard-state strong {
		font: 600 1.25rem/1.25 'Archivo SemiCondensed', sans-serif;
	}

	.leaderboard-state p {
		max-width: 48rem;
		margin: 0.55rem 0 0;
		color: var(--app-muted);
		font-size: 0.84rem;
		line-height: 1.5;
	}

	.leaderboard-state button {
		display: inline-block;
		margin-top: 1.1rem;
		padding: 0;
		border: 0;
		background: transparent;
		cursor: pointer;
		font-family: inherit;
		font-size: 0.78rem;
		font-weight: 600;
		text-decoration: none;
	}

	.leaderboard-state button:hover {
		color: var(--app-violet);
		text-decoration: underline;
		text-underline-offset: 0.2em;
	}

	.leaderboard > footer {
		display: flex;
		justify-content: space-between;
		gap: 2rem;
		padding: 1rem;
		background: #f8f8f5;
		color: var(--app-muted);
		font-size: 0.69rem;
		line-height: 1.4;
	}

	.leaderboard > footer p {
		max-width: 38rem;
		margin: 0;
	}

	@media (max-width: 820px) {
		.leaderboard-heading {
			align-items: flex-start;
			flex-direction: column;
			gap: 1.5rem;
		}

		.leaderboard > header,
		.leaderboard > footer {
			align-items: flex-start;
			flex-direction: column;
		}

		.snapshot {
			text-align: left;
		}
	}

	@media (max-width: 520px) {
		.sort-controls {
			align-items: stretch;
			flex-direction: column;
			width: 100%;
		}

		.sort-controls > span {
			margin-bottom: 0.25rem;
		}
	}
</style>
