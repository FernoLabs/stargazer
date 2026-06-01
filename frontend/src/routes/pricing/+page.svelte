<script lang="ts">
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import { firewatchPlans, type FireWatchPlan } from '$lib/monetization';

	function planAnchor(plan: FireWatchPlan) {
		if (plan.id === 'paid') return 'custom-plan';
		if (plan.id === 'pro') return 'pro-response';
		return 'shared-watch';
	}

	function setupCommand(plan: FireWatchPlan) {
		return plan.id === 'paid' ? '/config' : '/watch defi';
	}
</script>

<svelte:head>
	<title>FireWatch Pricing</title>
</svelte:head>

<div class="page-shell">
	<section class="panel intro-panel">
		<SectionHeader
			title="Pricing"
			description="Free is the default DeFi watcher. Paid is the custom auto-tracked watcher. On-demand is emergency on-chain Smart Wallet action on dedicated Ferno infra."
			level="1"
		/>
	</section>

	<section class="plans-grid" aria-label="FireWatch pricing tiers">
		{#each firewatchPlans as plan (plan.id)}
			<article class:featured={plan.id === 'paid'} class="plan-panel" id={planAnchor(plan)}>
				<div class="plan-head">
					<div>
						<h2>{plan.name}</h2>
						<p class="plan-description">{plan.description}</p>
					</div>
					<div class="price-block">
						<strong>{plan.price}</strong>
						<span>{plan.priceNote}</span>
					</div>
				</div>

				<ul class="feature-list">
					{#each plan.features as feature (feature)}
						<li>{feature}</li>
					{/each}
				</ul>

				{#if plan.id === 'pro'}
					<a class="plan-action" href={plan.ctaHref}>{plan.ctaLabel}</a>
				{:else}
					<p class="setup-hint">
						Add <a href="https://t.me/fernowatch_bot" rel="noreferrer" target="_blank">@fernowatch_bot</a>
						then send <code>{setupCommand(plan)}</code>.
					</p>
				{/if}
			</article>
		{/each}
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

	.panel,
	.plan-panel {
		background: #fff;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 10px;
	}

	.panel {
		padding: 1.05rem;
	}

	.plans-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 1rem;
		align-items: stretch;
	}

	.plan-panel {
		display: grid;
		align-content: start;
		gap: 0.8rem;
		padding: 1rem;
		min-width: 0;
		scroll-margin-top: 7.5rem;
	}

	.plan-panel.featured {
		border-color: rgba(189, 74, 23, 0.34);
		background: #fffdfb;
	}

	.plan-head {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 0.8rem;
		align-items: start;
	}

	h2,
	p,
	ul,
	li {
		margin: 0;
	}

	h2 {
		font-size: 1.02rem;
		line-height: 1.25;
	}

	.plan-description,
	li {
		color: #5f6c79;
		line-height: 1.56;
	}

	.plan-description {
		margin-top: 0.28rem;
		font-size: 0.88rem;
	}

	.price-block {
		display: grid;
		gap: 0.1rem;
		text-align: right;
		white-space: nowrap;
	}

	.price-block strong {
		color: #7d3a18;
		font-size: 1rem;
	}

	.price-block span {
		color: #62707d;
		font-size: 0.72rem;
		font-weight: 700;
	}

	.feature-list {
		display: grid;
		gap: 0.34rem;
		padding-left: 1.1rem;
	}

	.feature-list li {
		font-size: 0.88rem;
	}

	.setup-hint {
		padding-top: 0.68rem;
		border-top: 1px solid rgba(23, 32, 42, 0.08);
		color: #5f6c79;
		font-size: 0.84rem;
		line-height: 1.52;
	}

	.setup-hint a {
		color: #5d4c3e;
		font-weight: 760;
		text-underline-offset: 0.12em;
	}

	.setup-hint code {
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
		font-size: 0.8rem;
		color: #17202a;
	}

	.plan-action {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: fit-content;
		min-height: 2.3rem;
		padding: 0 0.86rem;
		border-radius: 8px;
		border: 1px solid #bd4a17;
		background: #bd4a17;
		color: #fff;
		font-size: 0.84rem;
		font-weight: 700;
		text-decoration: none;
	}

	@media (max-width: 980px) {
		.plans-grid {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}

	@media (max-width: 720px) {
		.page-shell {
			width: min(100vw - 1rem, var(--fw-page-max));
			padding-top: 0.8rem;
		}

		.plans-grid {
			grid-template-columns: 1fr;
		}

		.plan-head {
			grid-template-columns: 1fr;
		}

		.price-block {
			text-align: left;
		}
	}
</style>
