<script lang="ts">
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import TelegramCommandBuilder from '$lib/components/TelegramCommandBuilder.svelte';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	function botUsername() {
		return data.liveStatus?.telegram_bot?.username ?? null;
	}
</script>

<svelte:head>
	<title>Join the Watch • FireWatch</title>
</svelte:head>

<div class="page-shell">
	<section class="panel" id="command-builder">
		<TelegramCommandBuilder protocols={data.defiProtocols} botUsername={botUsername()} />
	</section>

	<section class="panel hero-panel">
		<SectionHeader
			title="Join the Watch"
			description="Free users can subscribe to the unfiltered DeFi feed. Custom Plan access unlocks filtered watch setup for paid users, Ferno members, and approved Superteam promos."
			level="1"
			href="/pricing"
			hrefLabel="Pricing"
		/>

		<div class="hero-grid">
			<div class="hero-copy">
				<p class="meta-line">Free DeFi feed · Custom Plan for filtered watches · Superteam promo is 12 months</p>
				<p>
					This page summarizes the community discussion around FireWatch and how the review
					layer works. FireWatch members do not replace the indexer. They add human triage on
					top of raw program, multisig, IDL, and dependency signals.
				</p>
			</div>

			<div class="status-card">
				<h2>What watch members do</h2>
				<ul>
					<li>Review sensitive alerts that deserve human context.</li>
					<li>Confirm whether a change looks routine, risky, or ecosystem-threatening.</li>
					<li>Upgrade an alert to <strong>Level Fire</strong> when a real threat is detected.</li>
				</ul>
			</div>
		</div>
	</section>

	<section class="grid two-up">
		<article class="panel">
			<h2>Triage model</h2>
			<ul>
				<li><strong>Watch</strong>: a signal worth reviewing, but not yet proven dangerous.</li>
				<li><strong>Review</strong>: strong concern or unusual activity with supporting context.</li>
				<li><strong>Level Fire</strong>: credible threat to protocol funds or the wider Solana ecosystem.</li>
			</ul>
		</article>

		<article class="panel">
			<h2>What Level Fire means</h2>
			<p>
				Level Fire is the highest community escalation. It means the reviewing group believes
				the alert indicates a real operational threat: malicious upgrade risk, governance
				compromise, unsafe signer change, exploit path, or another event that should trigger
				immediate defensive action.
			</p>
		</article>
	</section>

	<section class="panel">
		<h2>FAQ</h2>
		<div class="faq-list">
			<article class="faq-item">
				<h3>Who are watch members?</h3>
				<p>
					Watch members are the people allowed to discuss and triage alerts inside the
					FireWatch review layer. The current plan is to keep this group invite-only and
					FireWatch-first.
				</p>
			</article>

			<article class="faq-item">
				<h3>Who gets FireWatch free?</h3>
				<p>
					Everyone can use the unfiltered DeFi alert feed. Ferno members get Custom Plan
					access for free, and Superteam members can use an approved promo code for 12 months.
				</p>
			</article>

			<article class="faq-item">
				<h3>What is the community discussion about?</h3>
				<p>
					The discussion is about how to add human review without turning FireWatch into a
					noisy social feed. The goal is a small, trusted group that can assess alerts fast
					and raise the bar only when the threat is real.
				</p>
			</article>

			<article class="faq-item">
				<h3>Why not let everyone vote on alerts?</h3>
				<p>
					Because moderation matters. The current direction is a narrow review group with
					accountability, not an open popularity system.
				</p>
			</article>

			<article class="faq-item">
				<h3>What is Pro Response?</h3>
				<p>
					Pro Response is on-demand access to reviewed signals and a preauthorized Smart
					Account flow for withdrawal and redeployment after team review.
				</p>
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

	.panel {
		padding: 1.2rem 1.25rem;
		background: #fff;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 10px;
	}

	#command-builder {
		scroll-margin-top: 7.5rem;
	}

	.hero-panel {
		display: grid;
		gap: 0.9rem;
	}

	.hero-grid,
	.two-up {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 1rem;
	}

	.hero-copy,
	.status-card {
		display: grid;
		gap: 0.55rem;
	}

	.status-card {
		padding: 1rem;
		border-radius: 8px;
		background: #fbfaf8;
		border: 1px solid rgba(23, 32, 42, 0.08);
	}

	.meta-line {
		font-size: 0.82rem;
		font-weight: 600;
		color: #7b5b45;
	}

	h2,
	h3,
	p,
	ul {
		margin: 0;
	}

	p,
	li {
		color: #5f6c79;
		line-height: 1.65;
	}

	ul {
		padding-left: 1.2rem;
		display: grid;
		gap: 0.35rem;
	}

	.faq-list {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 0.9rem 1rem;
	}

	.faq-item {
		display: grid;
		gap: 0.35rem;
	}

	@media (max-width: 860px) {
		.hero-grid,
		.two-up,
		.faq-list {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 720px) {
		.page-shell {
			width: min(100vw - 1rem, var(--fw-page-max));
			padding-top: 0.8rem;
		}
	}
</style>
