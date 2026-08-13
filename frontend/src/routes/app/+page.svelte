<script lang="ts">
	import { resolve } from '$app/paths';
	import WalletSearch from '$lib/components/dashboard/WalletSearch.svelte';
	import { formatAddress, formatDatasetPeriod, formatExactCount } from '$lib/dashboard/format';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const yieldBloxWallet = 'GBO7VUL2TOKPWFAWKATIW7K3QYA7WQ63VDY5CAE6AFUUX6BHZBOC2WXC';
	let fallbackWallet = $derived(
		data.kind === 'ready' && !data.featuredWallet
			? (data.examples.wallets.find((wallet) => wallet.accountAddress !== yieldBloxWallet) ?? null)
			: null
	);
	let featuredWallet = $derived(data.kind === 'ready' ? data.featuredWallet : null);
	let source = $derived(data.kind === 'ready' ? data.examples.source : null);
	let indexedLedgerCount = $derived(
		source?.indexedLedgerStart !== undefined && source.indexedLedgerEnd !== undefined
			? source.indexedLedgerEnd - source.indexedLedgerStart + 1
			: null
	);
	let datasetPeriod = $derived(source ? formatDatasetPeriod(source.indexedFrom, source.asOf) : null);
</script>

<svelte:head>
	<title>Wallet dependency check · Stargazer demo</title>
	<meta
		name="description"
		content="Map a Stellar wallet to the protocols and contracts it depends on using real indexed mainnet data."
	/>
</svelte:head>

<header class="page-heading">
	<div>
		<h1>Wallet dependency check</h1>
		<p>
			See which Stellar protocols and contracts a wallet has used, then review relevant incidents,
			upgrades, and controller activity.
		</p>
	</div>
	<a href={resolve('/app/alerts')}>Browse all alerts →</a>
</header>

<section class="wallet-check" aria-labelledby="wallet-check-title">
	<h2 id="wallet-check-title">Enter a public Stellar address</h2>
	<WalletSearch
		description="Public G- or M-address only. No wallet connection, signature, or secret key."
		submitLabel="Open report"
	/>
</section>

{#if source}
	<section class="dataset" aria-label="Indexed dataset metrics">
		<dl>
			<div>
				<dt>Transactions scanned</dt>
				<dd>
					{source.indexedContractTransactionCount === undefined
						? '—'
						: formatExactCount(source.indexedContractTransactionCount)}
				</dd>
				<small>Contract-interacting transactions</small>
			</div>
			<div>
				<dt>Ledgers indexed</dt>
				<dd>{indexedLedgerCount === null ? '—' : formatExactCount(indexedLedgerCount)}</dd>
				<small>
					{source.indexedLedgerStart === undefined || source.indexedLedgerEnd === undefined
						? 'Range unavailable'
						: `${formatExactCount(source.indexedLedgerStart)}–${formatExactCount(source.indexedLedgerEnd)}`}
				</small>
			</div>
			<div>
				<dt>Data starts</dt>
				<dd>{datasetPeriod?.start ?? '—'}</dd>
				<small>First indexed ledger · UTC</small>
			</div>
			<div>
				<dt>Data ends</dt>
				<dd>{datasetPeriod?.end ?? '—'}</dd>
				<small>{datasetPeriod?.duration ?? 'Duration unavailable'} · UTC</small>
			</div>
		</dl>
	</section>
{:else}
	<p class="source-error" role="status">
		<strong>Indexed data is temporarily unavailable.</strong> {data.message}
	</p>
{/if}

<section class="examples" aria-labelledby="examples-title">
	<header>
		<div>
			<h2 id="examples-title">Try a real report</h2>
			<p>Open a wallet already present in the indexed archive.</p>
		</div>
	</header>
	<div class="example-list">
		<a href={resolve('/app/wallet/[address]', { address: yieldBloxWallet })}>
			<span>Incident exposure</span>
			<strong>YieldBlox retrospective</strong>
			<code>{formatAddress(yieldBloxWallet, { start: 12, end: 10 })}</code>
			<i aria-hidden="true">→</i>
		</a>

		{#if featuredWallet}
			<a href={resolve('/app/wallet/[address]', { address: featuredWallet.accountAddress })}>
				<span>Top wallet activity</span>
				<strong>#{featuredWallet.rank} by protocols used</strong>
				<code>{formatAddress(featuredWallet.accountAddress, { start: 12, end: 10 })}</code>
				<em>
					{formatExactCount(featuredWallet.protocolCount)} protocols ·
					{formatExactCount(featuredWallet.transactionCount)} transactions ·
					{featuredWallet.alertCount === null
						? 'alerts unavailable'
						: `${formatExactCount(featuredWallet.alertCount)} ${featuredWallet.alertCount === 1 ? 'alert' : 'alerts'}`}
				</em>
				<i aria-hidden="true">→</i>
			</a>
		{:else if fallbackWallet}
			<a href={resolve('/app/wallet/[address]', { address: fallbackWallet.accountAddress })}>
				<span>Indexed activity</span>
				<strong>Wallet activity report</strong>
				<code>{formatAddress(fallbackWallet.accountAddress, { start: 12, end: 10 })}</code>
				<i aria-hidden="true">→</i>
			</a>
		{/if}
	</div>
</section>

<footer class="scope-note">
	<strong>Demo data scope</strong>
	<p>
		{#if datasetPeriod}
			Stellar Mainnet archive coverage: {datasetPeriod.start} through {datasetPeriod.end}
			({datasetPeriod.duration}).
		{/if}
		Reports are reconstructed from this bounded archive. Retrospective findings show what Stargazer
		could have detected; they are not historical notification-delivery claims.
	</p>
</footer>

<style>
	.page-heading {
		display: flex;
		justify-content: space-between;
		gap: 2rem;
		align-items: end;
		padding: 0.5rem 0 1.75rem;
	}

	.page-heading h1 {
		margin: 0;
		font: 600 clamp(2rem, 4vw, 2.65rem)/1.05 'Archivo SemiCondensed', sans-serif;
		letter-spacing: -0.035em;
	}

	.page-heading p {
		max-width: 44rem;
		margin: 0.65rem 0 0;
		color: var(--app-muted);
		font-size: 0.95rem;
		line-height: 1.5;
	}

	.page-heading > a {
		flex: 0 0 auto;
		color: var(--app-violet);
		font-size: 0.84rem;
		font-weight: 600;
		text-decoration: none;
	}

	.wallet-check {
		padding: clamp(1.25rem, 3vw, 2rem);
		border: 1px solid var(--app-rule-strong);
		background: var(--app-paper);
	}

	.wallet-check h2 {
		margin: 0 0 1rem;
		font: 600 1.05rem/1.3 'Archivo SemiCondensed', sans-serif;
	}

	.dataset {
		margin-top: 1rem;
		border-top: 1px solid var(--app-rule-strong);
		border-bottom: 1px solid var(--app-rule-strong);
		background: var(--app-paper);
	}

	.dataset dl {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		margin: 0;
	}

	.dataset dl > div {
		min-width: 0;
		padding: 1rem;
		border-right: 1px solid var(--app-rule);
	}

	.dataset dl > div:last-child {
		border-right: 0;
	}

	.dataset dt,
	.dataset small {
		color: var(--app-muted);
		font-size: 0.7rem;
	}

	.dataset dt {
		font-weight: 600;
		letter-spacing: 0.025em;
		text-transform: uppercase;
	}

	.dataset dd {
		margin: 0.35rem 0 0.15rem;
		font: 600 1.12rem/1.2 'Archivo SemiCondensed', sans-serif;
	}

	.dataset small {
		display: block;
		font-family: 'Fragment Mono', monospace;
		line-height: 1.4;
	}

	.source-error {
		margin: 1rem 0 0;
		padding: 0.8rem 1rem;
		border-left: 2px solid var(--app-danger);
		background: #fff2f4;
		color: var(--app-muted);
		font-size: 0.82rem;
	}

	.source-error strong {
		color: var(--app-danger);
	}

	.examples {
		display: grid;
		grid-template-columns: minmax(13rem, 0.45fr) minmax(0, 1fr);
		margin-top: 1.75rem;
		border-top: 1px solid var(--app-rule-strong);
		border-bottom: 1px solid var(--app-rule-strong);
	}

	.examples > header {
		padding: 1.15rem 1.25rem 1.15rem 0;
		border-right: 1px solid var(--app-rule);
	}

	.examples h2 {
		margin: 0;
		font: 600 1rem/1.3 'Archivo SemiCondensed', sans-serif;
	}

	.examples header p {
		margin: 0.35rem 0 0;
		color: var(--app-muted);
		font-size: 0.78rem;
		line-height: 1.4;
	}

	.example-list {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		background: var(--app-paper);
	}

	.example-list a {
		position: relative;
		display: grid;
		gap: 0.2rem;
		padding: 1.15rem 3rem 1.15rem 1.25rem;
		border-right: 1px solid var(--app-rule);
		text-decoration: none;
	}

	.example-list a:last-child {
		border-right: 0;
	}

	.example-list a:hover {
		background: var(--app-violet-soft);
	}

	.example-list span {
		color: var(--app-muted);
		font-size: 0.72rem;
	}

	.example-list strong {
		font-size: 0.9rem;
	}

	.example-list code {
		margin-top: 0.35rem;
		color: var(--app-muted);
		font-size: 0.68rem;
	}

	.example-list em {
		margin-top: 0.15rem;
		color: var(--app-muted);
		font-size: 0.7rem;
		font-style: normal;
		line-height: 1.35;
	}

	.example-list i {
		position: absolute;
		top: 50%;
		right: 1.25rem;
		color: var(--app-violet);
		font-style: normal;
		transform: translateY(-50%);
	}

	.scope-note {
		display: grid;
		grid-template-columns: 10rem minmax(0, 1fr);
		gap: 1.5rem;
		margin-top: 2rem;
		padding-top: 1rem;
		border-top: 1px solid var(--app-rule);
	}

	.scope-note strong,
	.scope-note p {
		font-size: 0.78rem;
	}

	.scope-note p {
		max-width: 50rem;
		margin: 0;
		color: var(--app-muted);
		line-height: 1.5;
	}

	@media (max-width: 760px) {
		.page-heading {
			align-items: flex-start;
			flex-direction: column;
		}

		.dataset dl {
			grid-template-columns: repeat(2, 1fr);
		}

		.dataset dl > div:nth-child(2) {
			border-right: 0;
		}

		.dataset dl > div:nth-child(-n + 2) {
			border-bottom: 1px solid var(--app-rule);
		}

		.examples {
			grid-template-columns: 1fr;
		}

		.examples > header {
			padding-left: 1.25rem;
			border-right: 0;
			border-bottom: 1px solid var(--app-rule);
		}
	}

	@media (max-width: 540px) {
		.dataset dl,
		.example-list,
		.scope-note {
			grid-template-columns: 1fr;
		}

		.dataset dl > div,
		.dataset dl > div:nth-child(2),
		.example-list a {
			border-right: 0;
			border-bottom: 1px solid var(--app-rule);
		}

		.dataset dl > div:last-child,
		.example-list a:last-child {
			border-bottom: 0;
		}

		.scope-note {
			gap: 0.35rem;
		}
	}
</style>
