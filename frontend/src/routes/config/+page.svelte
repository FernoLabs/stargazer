<script lang="ts">
	import { resolve } from '$app/paths';
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const DEFAULT_BOT_USERNAME = 'fernowatch_bot';

	function botUsername() {
		const username = data.liveStatus?.telegram_bot?.username?.trim();
		return username || DEFAULT_BOT_USERNAME;
	}

	function addBotUrl() {
		return `https://t.me/${botUsername()}`;
	}

	function openAddBot() {
		window.open(addBotUrl(), '_blank', 'noopener,noreferrer');
	}
</script>

<svelte:head>
	<title>Connect FireWatch</title>
</svelte:head>

<div class="page-shell">
	<section class="panel connect-panel">
		<SectionHeader
			title="Connect FireWatch"
			description="Open your private setup from Telegram once. After that, this browser can return here to manage the same watch."
			level="1"
		/>

		{#if data.error}
			<p class="notice">Your saved browser session expired. Run <code>/config</code> again to reconnect.</p>
		{/if}

		<div class="connect-grid">
			<article>
				<strong>Telegram alerts</strong>
				<ol>
					<li>Open <code>@{botUsername()}</code> in Telegram.</li>
					<li>Run <code>/config</code> to open your private setup link.</li>
					<li>Use <code>/watch defi</code> for the curated public feed.</li>
				</ol>
				<button class="primary-action" type="button" onclick={openAddBot}>Open Telegram bot</button>
			</article>

			<article>
				<strong>Custom watch</strong>
				<ul>
					<li>Protocol, program, wallet, multisig, and account monitoring.</li>
					<li>Critical review workflow and routed delivery.</li>
					<li>Escalation paths for PagerDuty, Slack, Discord, or secure team channels.</li>
				</ul>
				<a class="secondary-action" href={resolve('/contact')}>Create protocol page</a>
			</article>
		</div>
	</section>
</div>

<style>
	.page-shell {
		width: min(var(--fw-page-max), calc(100vw - (var(--fw-page-gutter) * 2)));
		margin: 0 auto;
		padding: 1.25rem 0 3rem;
	}

	.panel {
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 8px;
		background: #fffdfa;
		padding: 1.4rem;
		box-shadow: 0 12px 32px rgba(23, 32, 42, 0.04);
	}

	.connect-panel {
		display: grid;
		gap: 1.1rem;
	}

	.notice {
		margin: 0;
		padding: 0.75rem 0.9rem;
		border: 1px solid rgba(156, 57, 41, 0.16);
		border-radius: 8px;
		background: rgba(156, 57, 41, 0.07);
		color: #8f3d19;
		font-weight: 800;
	}

	.connect-grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 0.9rem;
	}

	article {
		display: grid;
		align-content: start;
		gap: 0.85rem;
		min-height: 100%;
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 8px;
		background: #ffffff;
		padding: 1rem;
	}

	strong {
		color: #17202a;
		font-size: 1rem;
		font-weight: 820;
	}

	ol,
	ul {
		margin: 0;
		padding-left: 1.1rem;
		color: #5f6c79;
		font-weight: 650;
		line-height: 1.5;
	}

	li + li {
		margin-top: 0.4rem;
	}

	code {
		color: #17202a;
		font-weight: 900;
	}

	.primary-action,
	.secondary-action {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: fit-content;
		min-height: 2.8rem;
		margin-top: auto;
		padding: 0.7rem 1rem;
		border-radius: 8px;
		font-weight: 900;
		text-decoration: none;
	}

	.primary-action {
		border: 0;
		background: #ad4e22;
		color: #fffdfa;
		cursor: pointer;
		font: inherit;
	}

	.secondary-action {
		border: 1px solid rgba(23, 32, 42, 0.14);
		background: #fffdfa;
		color: #4f4038;
	}

	@media (max-width: 760px) {
		.page-shell {
			padding-top: 0.8rem;
		}

		.panel {
			padding: 0.95rem;
		}

		.connect-grid {
			grid-template-columns: 1fr;
		}

		article {
			gap: 0.76rem;
			padding: 0.92rem;
		}

		ol,
		ul {
			font-size: 0.94rem;
			line-height: 1.46;
		}

		.primary-action,
		.secondary-action {
			width: 100%;
		}
	}
</style>
