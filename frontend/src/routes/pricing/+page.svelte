<script lang="ts">
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import {
		firewatchPlans,
		proResponseSteps,
		type FireWatchPlan
	} from '$lib/monetization';

	function planAnchor(plan: FireWatchPlan) {
		if (plan.id === 'paid') return 'custom-plan';
		if (plan.id === 'pro') return 'pro-response';
		return 'shared-watch';
	}
</script>

<svelte:head>
	<title>FireWatch Pricing</title>
</svelte:head>

<div class="page-shell">
	<section class="panel intro-panel">
		<SectionHeader
			title="Pricing"
			description="Free users get the unfiltered DeFi alert feed. The Custom Plan unlocks paid watch customization, with Ferno members included and Superteam promo codes covering 12 months."
			level="1"
		/>
	</section>

	<section class="plans-grid" aria-label="FireWatch plans">
		{#each firewatchPlans as plan}
			<article class:featured={plan.id === 'paid'} class="plan-panel" id={planAnchor(plan)}>
				<div class="plan-head">
					<div>
						<h2>{plan.name}</h2>
						<p>{plan.description}</p>
					</div>
					<div class="price-block">
						<strong>{plan.price}</strong>
						<span>{plan.priceNote}</span>
					</div>
				</div>

				<ul class="feature-list">
					{#each plan.features as feature}
						<li>{feature}</li>
					{/each}
				</ul>

				<div class="limit-block">
					<strong>Access boundary</strong>
					<ul>
						{#each plan.limits as limit}
							<li>{limit}</li>
						{/each}
					</ul>
				</div>

				<a class="plan-action" href={plan.ctaHref}>{plan.ctaLabel}</a>
			</article>
		{/each}
	</section>

	<section class="grid two-up">
		<article class="panel">
			<h2>Custom Plan setup</h2>
			<ol>
				<li>Add the bot to Telegram and run <code>/config</code> as a group admin.</li>
				<li>Open the private setup link from the bot DM.</li>
				<li>Select protocols, programs, instruction tags, amount triggers, wallets, and .sol watches.</li>
				<li>Pay monthly or yearly; yearly access includes one month discounted.</li>
			</ol>
		</article>

		<article class="panel">
			<h2>What stays free</h2>
			<p>
				Unpaid access is the shared, unfiltered <code>/watch defi</code> feed. Ferno members
				and approved Superteam promo users can use Custom Plan access without paying during
				their eligibility window.
			</p>
		</article>
	</section>

	<section class="panel response-panel">
		<SectionHeader
			title="Pro response"
			description="Pro access is for teams that want reviewed signals and an authorized Smart Account path for emergency liquidity movement."
		/>

		<div class="response-flow">
			{#each proResponseSteps as step, index}
				<article class="response-step">
					<span>{index + 1}</span>
					<div>
						<h3>{step.title}</h3>
						<p>{step.description}</p>
					</div>
				</article>
			{/each}
		</div>

		<p class="response-note">
			Smart Account actions require explicit authorization, custody review, and a preapproved
			policy before FireWatch treats any fund movement as enabled.
		</p>
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
		gap: 0.9rem;
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
	h3,
	p,
	ul,
	ol {
		margin: 0;
	}

	h2 {
		font-size: 1.02rem;
		line-height: 1.25;
	}

	h3 {
		font-size: 0.92rem;
		line-height: 1.25;
	}

	p,
	li {
		color: #5f6c79;
		line-height: 1.58;
	}

	.plan-head p {
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

	.feature-list,
	.limit-block ul,
	ol {
		display: grid;
		gap: 0.34rem;
		padding-left: 1.1rem;
	}

	.feature-list li,
	.limit-block li,
	ol li {
		font-size: 0.88rem;
	}

	.limit-block {
		display: grid;
		gap: 0.36rem;
		padding-top: 0.78rem;
		border-top: 1px solid rgba(23, 32, 42, 0.08);
	}

	.limit-block strong {
		font-size: 0.82rem;
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

	.grid {
		display: grid;
		gap: 1rem;
	}

	.two-up {
		grid-template-columns: repeat(2, minmax(0, 1fr));
	}

	.response-panel {
		display: grid;
		gap: 0.9rem;
		scroll-margin-top: 7.5rem;
	}

	.response-flow {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: 0.7rem;
	}

	.response-step {
		display: grid;
		grid-template-columns: 26px minmax(0, 1fr);
		gap: 0.58rem;
		padding: 0.76rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		background: #fbfaf8;
	}

	.response-step span {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 26px;
		height: 26px;
		border-radius: 6px;
		background: #f3ece3;
		color: #bd4a17;
		font-size: 0.76rem;
		font-weight: 800;
	}

	.response-step p {
		margin-top: 0.18rem;
		font-size: 0.84rem;
	}

	.response-note {
		padding-top: 0.8rem;
		border-top: 1px solid rgba(23, 32, 42, 0.08);
		font-size: 0.88rem;
	}

	@media (max-width: 980px) {
		.plans-grid,
		.response-flow {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 820px) {
		.two-up {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 720px) {
		.page-shell {
			width: min(100vw - 1rem, var(--fw-page-max));
			padding-top: 0.8rem;
		}

		.plan-head {
			grid-template-columns: 1fr;
		}

		.price-block {
			text-align: left;
		}
	}
</style>
