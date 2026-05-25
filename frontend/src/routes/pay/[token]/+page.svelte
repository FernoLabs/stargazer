<script lang="ts">
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import { formatTimestamp, shortAddress } from '$lib/format';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	function userLabel() {
		if (data.user.telegram_username) return `@${data.user.telegram_username}`;
		if (data.user.telegram_user_id) return String(data.user.telegram_user_id);
		return data.user.label ?? 'FireWatch user';
	}
</script>

<svelte:head>
	<title>FireWatch Solana Pay</title>
</svelte:head>

<div class="page-shell">
	<section class="panel">
		<SectionHeader
			title="FireWatch payment"
			description="Pay with Solana Pay to activate Custom Plan access for pending custom watch rules."
			level="1"
			href="/pricing"
			hrefLabel="Pricing"
		/>
	</section>

	<section class="payment-grid">
		<article class="panel payment-panel">
			<div class="payment-head">
				<div>
					<span>Custom Plan</span>
					<h2>{data.payment_link.amount} SOL</h2>
					<p>{userLabel()}</p>
				</div>
				<span class={`status status-${data.payment_link.status}`}>{data.payment_link.status}</span>
			</div>

			<a class="pay-button" href={data.payment_link.solana_pay_url}>Open Solana Pay</a>

			<div class="command-box">
				<code>{data.payment_link.solana_pay_url}</code>
			</div>
		</article>

		<aside class="panel detail-panel">
			<h2>Payment details</h2>
			<ul>
				<li><strong>Recipient</strong><span>{shortAddress(data.payment_link.recipient)}</span></li>
				<li><strong>Reference</strong><span>{shortAddress(data.payment_link.reference)}</span></li>
				<li><strong>Memo</strong><span>{data.payment_link.memo}</span></li>
				<li><strong>Expires</strong><span>{formatTimestamp(data.payment_link.expires_at)}</span></li>
			</ul>
			<p>
				The watcher confirms finalized transactions to the FireWatch wallet with this reference.
				Custom alerts activate after confirmation.
			</p>
		</aside>
	</section>
</div>

<style>
	.page-shell {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem 0 2.4rem;
		display: grid;
		gap: 1rem;
	}

	.payment-grid {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(280px, 0.42fr);
		gap: 1rem;
	}

	.panel {
		padding: 1.15rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 10px;
		background: #fff;
	}

	.payment-panel,
	.detail-panel,
	.detail-panel ul {
		display: grid;
		gap: 0.85rem;
	}

	.payment-head {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 1rem;
	}

	h2,
	p,
	ul {
		margin: 0;
	}

	h2 {
		font-size: 1.4rem;
		line-height: 1.15;
	}

	p,
	li,
	.payment-head span {
		color: #5f6c79;
		line-height: 1.58;
	}

	.status,
	.pay-button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border-radius: 8px;
		font-size: 0.84rem;
		font-weight: 800;
		text-transform: capitalize;
	}

	.status {
		min-height: 2rem;
		padding: 0 0.72rem;
		border: 1px solid rgba(23, 32, 42, 0.1);
		background: #fbfaf8;
	}

	.status-paid {
		color: #216d4d;
	}

	.pay-button {
		min-height: 2.6rem;
		width: fit-content;
		padding: 0 1rem;
		background: #bd4a17;
		color: #fff;
		text-decoration: none;
	}

	.command-box {
		padding: 0.78rem 0.82rem;
		border-radius: 8px;
		border: 1px solid rgba(23, 32, 42, 0.1);
		background: #fbfaf8;
	}

	code {
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
		font-size: 0.82rem;
		overflow-wrap: anywhere;
	}

	.detail-panel ul {
		padding: 0;
		list-style: none;
	}

	.detail-panel li {
		display: flex;
		justify-content: space-between;
		gap: 1rem;
		border-bottom: 1px solid rgba(23, 32, 42, 0.08);
		padding-bottom: 0.5rem;
	}

	.detail-panel li span {
		text-align: right;
		overflow-wrap: anywhere;
	}

	@media (max-width: 800px) {
		.payment-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
