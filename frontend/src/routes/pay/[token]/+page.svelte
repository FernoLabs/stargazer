<script lang="ts">
	import { invalidateAll } from '$app/navigation';
	import { onMount } from 'svelte';
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import { formatTimestamp, shortAddress } from '$lib/format';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	type CopyTarget = 'uri' | 'recipient' | 'reference' | 'memo' | 'checklist';
	type CopyState = 'idle' | 'copied' | 'error';

	const statusRefreshMs = 10_000;
	const copyState = $state<Record<CopyTarget, CopyState>>({
		uri: 'idle',
		recipient: 'idle',
		reference: 'idle',
		memo: 'idle',
		checklist: 'idle'
	});

	function userLabel() {
		if (data.user.telegram_username) return `@${data.user.telegram_username}`;
		if (data.user.telegram_user_id) return String(data.user.telegram_user_id);
		return data.user.label ?? 'FireWatch user';
	}

	function statusLabel() {
		return data.payment_link.status.replaceAll('_', ' ');
	}

	function isPaid() {
		return data.payment_link.status === 'paid';
	}

	function isExpired() {
		return !isPaid() && data.payment_link.expires_at <= Math.floor(Date.now() / 1000);
	}

	function canPay() {
		return !isPaid() && !isExpired();
	}

	function checkoutStatusCopy() {
		if (isPaid()) {
			return 'Payment confirmed. FireWatch will activate pending custom watches for this Telegram user.';
		}

		if (isExpired()) {
			return 'This link has expired. Generate a fresh payment link from Telegram or the admin console.';
		}

		return 'Pending payment. Keep this page open after paying; status refreshes automatically.';
	}

	function setupChecklist() {
		return [
			`FireWatch Custom Plan setup for ${userLabel()}`,
			`Amount: ${data.payment_link.amount} SOL`,
			`Recipient: ${data.payment_link.recipient}`,
			`Reference: ${data.payment_link.reference}`,
			`Memo: ${data.payment_link.memo}`,
			'After payment: return to Telegram and keep your /watch wallet draft ready.'
		].join('\n');
	}

	function fallbackCopy(text: string) {
		const textarea = document.createElement('textarea');
		textarea.value = text;
		textarea.setAttribute('readonly', 'true');
		textarea.style.position = 'fixed';
		textarea.style.opacity = '0';
		document.body.append(textarea);
		textarea.select();
		const copied = document.execCommand('copy');
		textarea.remove();
		if (!copied) throw new Error('Copy command failed');
	}

	async function writeClipboard(text: string) {
		if (navigator.clipboard?.writeText) {
			try {
				await navigator.clipboard.writeText(text);
				return;
			} catch {
				// Fall back for embedded browsers that block async clipboard writes.
			}
		}

		fallbackCopy(text);
	}

	function resetCopyState(target: CopyTarget) {
		window.setTimeout(() => {
			copyState[target] = 'idle';
		}, 1800);
	}

	function copyButtonLabel(target: CopyTarget, idleLabel: string) {
		if (copyState[target] === 'copied') return 'Copied';
		if (copyState[target] === 'error') return 'Copy failed';
		return idleLabel;
	}

	async function copyText(target: CopyTarget, text: string) {
		if (!text) return;

		try {
			await writeClipboard(text);
			copyState[target] = 'copied';
			resetCopyState(target);
		} catch {
			copyState[target] = 'error';
			resetCopyState(target);
		}
	}

	onMount(() => {
		if (isPaid()) return;

		const interval = window.setInterval(() => {
			if (isPaid() || isExpired()) {
				window.clearInterval(interval);
				return;
			}

			if (document.visibilityState === 'visible') {
				void invalidateAll();
			}
		}, statusRefreshMs);

		return () => window.clearInterval(interval);
	});
</script>

<svelte:head>
	<title>FireWatch Solana Pay</title>
</svelte:head>

<div class="page-shell">
	<section class="panel intro-panel">
		<SectionHeader
			title="FireWatch payment"
			description="Activate Custom Plan access for pending custom wallet, protocol, or program watches."
			level="1"
			href="/pricing"
			hrefLabel="Pricing"
		/>
		<p class:confirmed={isPaid()} class:error={isExpired()} class="status-note">
			{checkoutStatusCopy()}
		</p>
	</section>

	<section class="payment-grid">
		<article class="panel payment-panel">
			<div class="payment-head">
				<div>
					<span>Custom Plan</span>
					<h2>{data.payment_link.amount} SOL</h2>
					<p>{userLabel()}</p>
				</div>
				<span class={`status status-${data.payment_link.status}`}>{statusLabel()}</span>
			</div>

			<div class="action-row">
				{#if canPay()}
					<a class="pay-button" href={data.payment_link.solana_pay_url}>Open wallet</a>
				{/if}
				<button
					class="secondary-button"
					type="button"
					onclick={() => copyText('uri', data.payment_link.solana_pay_url)}
				>
					{copyButtonLabel('uri', 'Copy pay URI')}
				</button>
				<button class="secondary-button" type="button" onclick={() => invalidateAll()}>
					Refresh status
				</button>
			</div>

			<div class="command-box">
				<code>{data.payment_link.solana_pay_url}</code>
			</div>

			<div class="wallet-help">
				<h2>Wallet setup</h2>
				<ol>
					<li>Open this page on the device where your Solana wallet is installed.</li>
					<li>Use <strong>Open wallet</strong>, or copy the URI into a wallet that supports Solana Pay.</li>
					<li>Confirm the amount, recipient, memo, and reference before approving.</li>
				</ol>
			</div>
		</article>

		<aside class="panel detail-panel">
			<h2>Payment details</h2>
			<ul>
				<li>
					<strong>Recipient</strong>
					<span>{shortAddress(data.payment_link.recipient)}</span>
					<button type="button" onclick={() => copyText('recipient', data.payment_link.recipient)}>
						{copyButtonLabel('recipient', 'Copy')}
					</button>
				</li>
				<li>
					<strong>Reference</strong>
					<span>{shortAddress(data.payment_link.reference)}</span>
					<button type="button" onclick={() => copyText('reference', data.payment_link.reference)}>
						{copyButtonLabel('reference', 'Copy')}
					</button>
				</li>
				<li>
					<strong>Memo</strong>
					<span>{data.payment_link.memo}</span>
					<button type="button" onclick={() => copyText('memo', data.payment_link.memo)}>
						{copyButtonLabel('memo', 'Copy')}
					</button>
				</li>
				<li><strong>Expires</strong><span>{formatTimestamp(data.payment_link.expires_at)}</span></li>
			</ul>
			<p>
				The watcher confirms finalized transactions to the FireWatch wallet with this reference.
				Custom alerts activate after confirmation.
			</p>
		</aside>
	</section>

	<section class="panel onboarding-panel">
		<div class="onboarding-head">
			<h2>Custom watch onboarding</h2>
			<button
				class="secondary-button"
				type="button"
				onclick={() => copyText('checklist', setupChecklist())}
			>
				{copyButtonLabel('checklist', 'Copy checklist')}
			</button>
		</div>

		<div class="onboarding-steps">
			<article>
				<span>1</span>
				<div>
					<strong>Draft the rule</strong>
					<p>
						Use the command builder for a wallet rule such as
						<code>/watch wallet &lt;wallet&gt; any-transfer</code>.
					</p>
					<a href="/join-the-watch#command-builder">Open command builder</a>
				</div>
			</article>
			<article>
				<span>2</span>
				<div>
					<strong>Pay this link</strong>
					<p>
						Pay the exact SOL amount with the reference intact. FireWatch uses that reference
						to match the transaction.
					</p>
				</div>
			</article>
			<article>
				<span>3</span>
				<div>
					<strong>Wait for activation</strong>
					<p>
						After confirmation, pending wallet and filtered watch rules move from paused to active.
					</p>
				</div>
			</article>
		</div>
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

	.intro-panel {
		display: grid;
		gap: 0.75rem;
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
	.detail-panel ul,
	.wallet-help,
	.onboarding-panel {
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
	ul,
	ol {
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

	.status-note {
		padding: 0.72rem 0.82rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		background: #fbfaf8;
		font-size: 0.9rem;
		font-weight: 650;
	}

	.status-note.confirmed {
		border-color: rgba(33, 109, 77, 0.18);
		background: rgba(33, 109, 77, 0.08);
		color: #216d4d;
	}

	.status-note.error {
		border-color: rgba(156, 57, 41, 0.18);
		background: rgba(156, 57, 41, 0.08);
		color: #9c3929;
	}

	.status,
	.pay-button,
	.secondary-button,
	.detail-panel button {
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

	.secondary-button,
	.detail-panel button {
		min-height: 2.3rem;
		padding: 0 0.82rem;
		border: 1px solid rgba(23, 32, 42, 0.12);
		background: #fff;
		color: #556473;
		font: inherit;
		font-size: 0.82rem;
		font-weight: 800;
		cursor: pointer;
		text-decoration: none;
	}

	.action-row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.52rem;
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

	.wallet-help {
		padding-top: 0.85rem;
		border-top: 1px solid rgba(23, 32, 42, 0.08);
	}

	.wallet-help ol {
		display: grid;
		gap: 0.34rem;
		padding-left: 1.1rem;
	}

	.detail-panel ul {
		padding: 0;
		list-style: none;
	}

	.detail-panel li {
		display: grid;
		grid-template-columns: minmax(5.5rem, 0.32fr) minmax(0, 1fr) auto;
		align-items: center;
		gap: 1rem;
		border-bottom: 1px solid rgba(23, 32, 42, 0.08);
		padding-bottom: 0.5rem;
	}

	.detail-panel li span {
		text-align: right;
		overflow-wrap: anywhere;
	}

	.onboarding-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		flex-wrap: wrap;
	}

	.onboarding-steps {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 0.75rem;
	}

	.onboarding-steps article {
		display: grid;
		grid-template-columns: 2rem minmax(0, 1fr);
		gap: 0.62rem;
		padding: 0.8rem;
		border-radius: 8px;
		border: 1px solid rgba(23, 32, 42, 0.08);
		background: #fbfaf8;
	}

	.onboarding-steps article > span {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 2rem;
		height: 2rem;
		border-radius: 8px;
		background: #f3ece3;
		color: #bd4a17;
		font-weight: 800;
	}

	.onboarding-steps div {
		display: grid;
		gap: 0.25rem;
	}

	.onboarding-steps a {
		width: fit-content;
		color: #7b5b45;
		font-size: 0.84rem;
		font-weight: 800;
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	@media (max-width: 800px) {
		.payment-grid,
		.onboarding-steps {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 620px) {
		.detail-panel li {
			grid-template-columns: 1fr;
			gap: 0.34rem;
		}

		.detail-panel li span {
			text-align: left;
		}

		.pay-button,
		.secondary-button {
			width: 100%;
		}
	}
</style>
