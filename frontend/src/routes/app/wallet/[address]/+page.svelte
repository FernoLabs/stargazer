<script lang="ts">
	import { resolve } from '$app/paths';
	import AlertReport from '$lib/components/dashboard/AlertReport.svelte';
	import ProtocolTable from '$lib/components/dashboard/ProtocolTable.svelte';
	import WalletSearch from '$lib/components/dashboard/WalletSearch.svelte';
	import { formatAddress, formatDatasetPeriod, formatExactCount } from '$lib/dashboard/format';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	let copied = $state(false);
	let datasetPeriod = $derived(
		data.kind === 'ready'
			? formatDatasetPeriod(data.dashboard.source.indexedFrom, data.dashboard.source.asOf)
			: null
	);

	async function copyAddress(address: string) {
		await navigator.clipboard.writeText(address);
		copied = true;
		window.setTimeout(() => (copied = false), 1600);
	}
</script>

<svelte:head>
	<title>
		{data.kind === 'ready' ? formatAddress(data.dashboard.query.inputAddress) : 'Wallet report'} ·
		Stargazer
	</title>
	<meta
		name="description"
		content="A real archive-backed report of a Stellar wallet's protocol dependencies and relevant findings."
	/>
</svelte:head>

<a class="back-link" href={resolve('/app')}>← Check another wallet</a>

{#if data.kind === 'invalid'}
	<section class="message-state" aria-labelledby="invalid-title">
		<h1 id="invalid-title">Invalid Stellar address</h1>
		<p>{data.message}</p>
		<div class="state-search"><WalletSearch initialValue={data.input} /></div>
	</section>
{:else if data.kind === 'unavailable'}
	<section class="message-state" aria-labelledby="unavailable-title">
		<h1 id="unavailable-title">Wallet report unavailable</h1>
		<p>{data.message} No sample data has been substituted.</p>
		<code>{data.address}</code>
		<div class="state-actions">
			<a class="primary-link" href={resolve('/app/wallet/[address]', { address: data.address })}>
				Retry report
			</a>
			<a href={resolve('/app')}>Search another wallet</a>
		</div>
	</section>
{:else}
	<header class="wallet-header">
		<div>
			<h1>Wallet report</h1>
			<div class="address-row">
				<code>{data.dashboard.query.inputAddress}</code>
				<button type="button" onclick={() => copyAddress(data.dashboard.query.inputAddress)}>
					{copied ? 'Copied' : 'Copy'}
				</button>
			</div>
			{#if data.walletKind === 'muxed-account'}
				<p class="muxed-note">
					Base account <code>{formatAddress(data.dashboard.query.accountAddress)}</code>
					{#if data.dashboard.query.muxedId}· muxed ID {data.dashboard.query.muxedId}{/if}
				</p>
			{/if}
		</div>
		<p class="archive-range">
			<strong>{datasetPeriod?.range ?? 'Archive period unavailable'}</strong>
			{#if
				data.dashboard.source.indexedLedgerStart !== undefined &&
				data.dashboard.source.indexedLedgerEnd !== undefined}
				<span>
					{datasetPeriod?.duration ?? 'Bounded archive'} · ledgers {formatExactCount(data.dashboard.source.indexedLedgerStart)}–{formatExactCount(
						data.dashboard.source.indexedLedgerEnd
					)}
				</span>
			{/if}
		</p>
	</header>

	<section class="wallet-metrics" aria-labelledby="wallet-activity-title">
		<header>
			<div>
				<h2 id="wallet-activity-title">Wallet activity</h2>
				<p>Exact contract interactions recorded in the indexed archive.</p>
			</div>
			<a href={resolve('/app/leaderboard?sort=protocols')}>View top wallet activity →</a>
		</header>
		<dl>
			<div>
				<dt>Contract transactions</dt>
				<dd>
					{data.dashboard.summary.transactionCount === null
						? '—'
						: formatExactCount(data.dashboard.summary.transactionCount)}
				</dd>
				{#if
					data.dashboard.summary.transactionRank !== null &&
					data.dashboard.summary.rankedWalletCount !== null}
					<small>
						#{formatExactCount(data.dashboard.summary.transactionRank)} of
						{formatExactCount(data.dashboard.summary.rankedWalletCount)} wallets
					</small>
				{/if}
			</div>
			<div>
				<dt>Protocols used</dt>
				<dd>{formatExactCount(data.dashboard.summary.protocolCount)}</dd>
				{#if
					data.dashboard.summary.protocolRank !== null &&
					data.dashboard.summary.rankedWalletCount !== null}
					<small>
						#{formatExactCount(data.dashboard.summary.protocolRank)} of
						{formatExactCount(data.dashboard.summary.rankedWalletCount)} wallets
					</small>
				{/if}
			</div>
			<div>
				<dt>Contracts used</dt>
				<dd>{formatExactCount(data.dashboard.summary.contractCount)}</dd>
				<small>Exact on-chain relationships</small>
			</div>
			<div>
				<dt>Historical alerts</dt>
				<dd>
					{data.dashboard.alerts.available
						? formatExactCount(data.dashboard.alerts.items.length)
						: '—'}
				</dd>
				<small>Retrospective incident matches</small>
			</div>
		</dl>
	</section>

	<section class="dependencies" aria-labelledby="dependencies-title">
		<header>
			<div>
				<h2 id="dependencies-title">Protocol dependencies</h2>
				<p>
					Verified registry matches use a protocol name; all other relationships stay visible under
					their C-address.
				</p>
			</div>
			<span>
				{formatExactCount(data.dashboard.summary.protocolCount)} named ·
				{formatExactCount(data.dashboard.summary.unattributedContractCount)} unattributed
			</span>
		</header>
		<ProtocolTable protocols={data.dashboard.protocols} contracts={data.dashboard.contracts} />

		{#if data.dashboard.contracts.nextCursor || data.contractCursor}
			<nav class="contract-pagination" aria-label="Contract relationship pages">
				<p>Showing {formatExactCount(data.dashboard.contracts.items.length)} relationships.</p>
				<div>
					{#if data.contractCursor}
						<a href={resolve('/app/wallet/[address]', { address: data.dashboard.query.inputAddress })}>
							First page
						</a>
					{/if}
					{#if data.dashboard.contracts.nextCursor}
						<form
							method="GET"
							action={resolve('/app/wallet/[address]', {
								address: data.dashboard.query.inputAddress
							})}
						>
							<input type="hidden" name="cursor" value={data.dashboard.contracts.nextCursor} />
							<button type="submit">Next contracts →</button>
						</form>
					{/if}
				</div>
			</nav>
		{/if}
	</section>

	<section class="findings" aria-label="Wallet findings">
		<AlertReport wallet={data.dashboard} />
	</section>

	<aside class="telegram-row" aria-labelledby="telegram-title">
		<div>
			<h2 id="telegram-title">Get alerts in Telegram</h2>
			<p>Choose critical or informational changes and associate this public wallet with a chat.</p>
		</div>
		<div>
			<span>Configuration preview · delivery and payment are not connected</span>
			<a href={resolve('/app/watch/[address]', { address: data.dashboard.query.inputAddress })}>
				Configure Telegram →
			</a>
		</div>
	</aside>

	<footer class="source-note">
		<strong>Data scope</strong>
		<span>
			{data.dashboard.source.label}.
			{#if datasetPeriod}
				Stellar Mainnet coverage runs from {datasetPeriod.start} through {datasetPeriod.end}
				({datasetPeriod.duration}).
			{/if}
			{data.dashboard.source.disclaimer} Historical incident matches are retrospective; continuous
			notification delivery is not active in this demo.
		</span>
	</footer>
{/if}

<style>
	.back-link {
		display: inline-block;
		margin-bottom: 1.35rem;
		color: var(--app-muted);
		font-size: 0.78rem;
		font-weight: 600;
		text-decoration: none;
	}

	.wallet-header {
		display: flex;
		justify-content: space-between;
		gap: 2rem;
		align-items: end;
	}

	.wallet-header h1,
	.message-state h1 {
		margin: 0;
		font: 600 clamp(2rem, 4vw, 2.65rem)/1.05 'Archivo SemiCondensed', sans-serif;
		letter-spacing: -0.035em;
	}

	.address-row {
		display: flex;
		gap: 0.55rem;
		align-items: center;
		min-width: 0;
		margin-top: 0.55rem;
	}

	.address-row code,
	.message-state code {
		overflow-wrap: anywhere;
		color: var(--app-muted);
		font: 0.7rem/1.45 'Fragment Mono', monospace;
	}

	.address-row button {
		flex: 0 0 auto;
		padding: 0.25rem 0.45rem;
		border: 1px solid var(--app-rule-strong);
		border-radius: 3px;
		background: transparent;
		color: var(--app-muted);
		cursor: pointer;
		font-size: 0.68rem;
	}

	.address-row button:hover {
		border-color: var(--app-violet);
		color: var(--app-violet);
	}

	.muxed-note {
		margin: 0.4rem 0 0;
		color: var(--app-muted);
		font-size: 0.76rem;
	}

	.archive-range {
		display: grid;
		gap: 0.2rem;
		margin: 0;
		color: var(--app-muted);
		font: 0.68rem/1.4 'Fragment Mono', monospace;
		text-align: right;
	}

	.archive-range strong {
		color: var(--app-ink);
	}

	.wallet-metrics {
		margin-top: 1.25rem;
		border-top: 1px solid var(--app-rule-strong);
		border-bottom: 1px solid var(--app-rule-strong);
		background: var(--app-paper);
	}

	.wallet-metrics > header {
		display: flex;
		justify-content: space-between;
		gap: 1.5rem;
		align-items: center;
		padding: 0.75rem 1rem;
		border-bottom: 1px solid var(--app-rule);
	}

	.wallet-metrics h2,
	.wallet-metrics p {
		margin: 0;
	}

	.wallet-metrics h2 {
		font: 600 1rem/1.3 'Archivo SemiCondensed', sans-serif;
	}

	.wallet-metrics p {
		margin-top: 0.15rem;
		color: var(--app-muted);
		font-size: 0.76rem;
	}

	.wallet-metrics > header a {
		color: var(--app-violet);
		font-size: 0.76rem;
		font-weight: 600;
		text-decoration: none;
		white-space: nowrap;
	}

	.wallet-metrics dl {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		margin: 0;
	}

	.wallet-metrics dl > div {
		padding: 0.85rem 1rem;
		border-right: 1px solid var(--app-rule);
	}

	.wallet-metrics dl > div:last-child {
		border-right: 0;
	}

	.wallet-metrics dt {
		color: var(--app-muted);
		font-size: 0.72rem;
	}

	.wallet-metrics dd {
		margin: 0.2rem 0 0;
		font: 600 1rem/1.2 'Fragment Mono', monospace;
	}

	.wallet-metrics small {
		display: block;
		margin-top: 0.25rem;
		color: var(--app-muted);
		font-size: 0.68rem;
		line-height: 1.35;
	}

	.dependencies,
	.findings {
		margin-top: 2rem;
	}

	.dependencies > header {
		display: flex;
		justify-content: space-between;
		gap: 1.5rem;
		align-items: end;
		padding-bottom: 0.75rem;
		border-bottom: 1px solid var(--app-ink);
	}

	.dependencies h2,
	.telegram-row h2 {
		margin: 0;
		font: 600 1.15rem/1.3 'Archivo SemiCondensed', sans-serif;
	}

	.dependencies header p,
	.telegram-row p {
		max-width: 50rem;
		margin: 0.25rem 0 0;
		color: var(--app-muted);
		font-size: 0.8rem;
		line-height: 1.45;
	}

	.dependencies header > span {
		color: var(--app-muted);
		font: 0.7rem/1.4 'Fragment Mono', monospace;
		white-space: nowrap;
	}

	.contract-pagination {
		display: flex;
		justify-content: space-between;
		gap: 1rem;
		align-items: center;
		padding: 0.75rem 1rem;
		border-bottom: 1px solid var(--app-rule);
		background: var(--app-paper);
		font-size: 0.76rem;
	}

	.contract-pagination p,
	.contract-pagination form {
		margin: 0;
	}

	.contract-pagination p {
		color: var(--app-muted);
	}

	.contract-pagination div {
		display: flex;
		gap: 1rem;
	}

	.contract-pagination a,
	.contract-pagination button {
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--app-violet);
		cursor: pointer;
		font: inherit;
		font-weight: 600;
		text-decoration: none;
	}

	.telegram-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 2rem;
		align-items: center;
		margin-top: 2rem;
		padding: 1rem;
		border-top: 1px solid var(--app-ink);
		border-bottom: 1px solid var(--app-rule);
		background: var(--app-violet-soft);
	}

	.telegram-row > div:last-child {
		display: grid;
		gap: 0.45rem;
		justify-items: end;
	}

	.telegram-row span {
		color: var(--app-muted);
		font-size: 0.7rem;
	}

	.telegram-row a {
		padding: 0.65rem 0.8rem;
		background: var(--app-ink);
		color: white;
		font-size: 0.78rem;
		font-weight: 600;
		text-decoration: none;
	}

	.source-note {
		display: grid;
		grid-template-columns: 7rem minmax(0, 1fr);
		gap: 1rem;
		margin-top: 2rem;
		padding-top: 0.9rem;
		border-top: 1px solid var(--app-rule);
		color: var(--app-muted);
		font-size: 0.76rem;
		line-height: 1.45;
	}

	.source-note strong {
		color: var(--app-ink);
	}

	.message-state {
		max-width: 45rem;
		padding: 2rem 0;
	}

	.message-state > p {
		margin: 0.7rem 0;
		color: var(--app-muted);
	}

	.state-search {
		margin-top: 1.25rem;
	}

	.state-actions {
		display: flex;
		gap: 1rem;
		align-items: center;
		margin-top: 1rem;
	}

	.state-actions a {
		font-size: 0.78rem;
		font-weight: 600;
		text-decoration: none;
	}

	.state-actions .primary-link {
		padding: 0.65rem 0.8rem;
		background: var(--app-ink);
		color: white;
	}

	@media (max-width: 700px) {
		.wallet-header,
		.wallet-metrics > header,
		.dependencies > header,
		.telegram-row {
			align-items: flex-start;
			grid-template-columns: 1fr;
			flex-direction: column;
		}

		.archive-range {
			text-align: left;
		}

		.wallet-metrics dl {
			grid-template-columns: repeat(2, 1fr);
		}

		.wallet-metrics dl > div:nth-child(2) {
			border-right: 0;
		}

		.wallet-metrics dl > div:nth-child(-n + 2) {
			border-bottom: 1px solid var(--app-rule);
		}

		.telegram-row {
			display: grid;
		}

		.telegram-row > div:last-child {
			justify-items: start;
		}

		.contract-pagination {
			align-items: flex-start;
			flex-direction: column;
		}
	}
</style>
