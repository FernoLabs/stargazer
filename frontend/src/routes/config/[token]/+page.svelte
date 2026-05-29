<script lang="ts">
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import TelegramCommandBuilder from '$lib/components/TelegramCommandBuilder.svelte';
	import { formatTimestamp } from '$lib/format';
	import type { PageData } from './$types';

	let { data, form }: { data: PageData; form: { error?: string } | null } = $props();

	function userLabel() {
		if (!data.user) return 'Telegram admin';
		if (data.user.telegram_username) return `@${data.user.telegram_username}`;
		if (data.user.telegram_user_id) return String(data.user.telegram_user_id);
		return data.user.label ?? 'Telegram admin';
	}

	function chatLabel() {
		return data.session?.chat_title ?? 'Telegram chat';
	}

	function botUsername() {
		return data.liveStatus?.telegram_bot?.username ?? null;
	}
</script>

<svelte:head>
	<title>Configure FireWatch</title>
</svelte:head>

<div class="page-shell">
	{#if data.error || !data.session || !data.user}
		<section class="panel">
			<SectionHeader
				title="Config link expired"
				description={data.error ?? 'Run /config again in Telegram to receive a fresh private setup link.'}
				level="1"
				href="/"
				hrefLabel="Home"
			/>
		</section>
	{:else}
		<section class="panel intro-panel">
			<SectionHeader
				title={`Configure ${chatLabel()}`}
				description={`Signed in from Telegram as ${userLabel()}. This setup link expires ${formatTimestamp(data.session.expires_at)}.`}
				level="1"
			/>
			<div class="session-grid">
				<div>
					<strong>Admin</strong>
					<span>{userLabel()}</span>
				</div>
				<div>
					<strong>Chat</strong>
					<span>{chatLabel()}</span>
				</div>
				<div>
					<strong>Access</strong>
					<span>Custom bot setup</span>
				</div>
			</div>
		</section>

		<section class="panel">
			<TelegramCommandBuilder
				protocols={data.defiProtocols}
				botUsername={botUsername()}
				defaultPlan="paid"
				defaultMode="protocol"
				showPlanPanel={false}
				heading="Rule setup"
				description="Select a protocol, program, wallet rule, instruction tag, amount trigger, or .sol watch before activating Custom Plan access."
			/>
		</section>

		<section class="checkout-grid" aria-label="Checkout options">
			<article class="panel checkout-card">
				<div>
					<h2>Monthly</h2>
					<p>Custom bot configuration with protocol, program, wallet, tag, amount, and .sol watches.</p>
				</div>
				<form method="POST" action="?/create_checkout">
					<input name="billing" type="hidden" value="month" />
					<button type="submit">Pay monthly</button>
				</form>
			</article>

			<article class="panel checkout-card preferred">
				<div>
					<h2>Yearly</h2>
					<p>One year of Custom Plan access with one month discounted.</p>
				</div>
				<form method="POST" action="?/create_checkout">
					<input name="billing" type="hidden" value="year" />
					<button type="submit">Pay yearly</button>
				</form>
			</article>
		</section>

		{#if form?.error}
			<p class="flash error">{form.error}</p>
		{/if}
	{/if}
</div>

<style>
	.page-shell {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem 0 2.4rem;
		display: grid;
		gap: 1rem;
	}

	.panel {
		padding: 1.05rem;
		background: #fff;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 10px;
	}

	.intro-panel,
	.checkout-card {
		display: grid;
		gap: 0.9rem;
	}

	.session-grid,
	.checkout-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 0.7rem;
	}

	.session-grid div {
		display: grid;
		gap: 0.12rem;
		padding: 0.72rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		background: #fbfaf8;
	}

	.session-grid strong,
	.session-grid span {
		overflow-wrap: anywhere;
	}

	.session-grid strong {
		font-size: 0.8rem;
	}

	.session-grid span,
	p {
		color: #5f6c79;
		line-height: 1.55;
	}

	.checkout-grid {
		grid-template-columns: repeat(2, minmax(0, 1fr));
	}

	.checkout-card.preferred {
		border-color: rgba(189, 74, 23, 0.34);
		background: #fffdfb;
	}

	h2,
	p {
		margin: 0;
	}

	h2 {
		font-size: 1.05rem;
	}

	button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 2.35rem;
		padding: 0 0.88rem;
		border: 1px solid #bd4a17;
		border-radius: 8px;
		background: #bd4a17;
		color: #fff;
		font: inherit;
		font-size: 0.84rem;
		font-weight: 800;
		cursor: pointer;
	}

	.flash {
		padding: 0.75rem 0.85rem;
		border: 1px solid rgba(156, 57, 41, 0.18);
		border-radius: 8px;
		background: rgba(156, 57, 41, 0.08);
		color: #9c3929;
		font-weight: 700;
	}

	@media (max-width: 840px) {
		.session-grid,
		.checkout-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
