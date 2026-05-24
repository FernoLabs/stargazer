<script lang="ts">
	import { protocolPageHref } from '$lib/catalog';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import towerIcon from '$lib/assets/firewatch-tower-icon.png';
	import type { DefiProtocolDescriptor } from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	type MonitorBadge = {
		label: string;
		count: number;
	};

	function botHandle() {
		const username = data.liveStatus?.telegram_bot?.username;
		return username ? `@${username}` : null;
	}

	function botLabel() {
		const bot = data.liveStatus?.telegram_bot;
		if (!bot) return 'Telegram bot not configured';
		return botHandle() ? `${bot.first_name} (${botHandle()})` : bot.first_name;
	}

	function formatCount(value: number | null | undefined) {
		return new Intl.NumberFormat('en-US').format(value ?? 0);
	}

	function totalAdminInstructionCount() {
		return data.defiProtocols.reduce(
			(count, protocol) => count + protocol.admin_instruction_filters.length,
			0
		);
	}

	function totalInstructionCount() {
		return data.defiProtocols.reduce(
			(count, protocol) => count + protocol.catalog_instruction_count,
			0
		);
	}

	function totalMetricTriggerCount() {
		return data.defiProtocols.reduce(
			(count, protocol) => count + protocol.amount_triggers.length,
			0
		);
	}

	function totalSolDomainCount() {
		return data.defiProtocols.reduce(
			(count, protocol) => count + protocol.sol_domains.length,
			0
		);
	}

	function monitorLegendBadges(): MonitorBadge[] {
		return [
			{ label: 'SNS', count: totalSolDomainCount() },
			{ label: 'Squads', count: 1 },
			{ label: 'Program', count: data.trackedProgramCount },
			{ label: 'Instruction', count: totalInstructionCount() },
			{ label: 'Admin instruction', count: totalAdminInstructionCount() },
			{ label: 'Amount trigger', count: totalMetricTriggerCount() }
		];
	}

	function protocolMonitorBadges(protocol: DefiProtocolDescriptor): MonitorBadge[] {
		const badges: MonitorBadge[] = [];

		if (protocol.programs.length > 0) {
			badges.push({ label: 'Program', count: protocol.programs.length });
		}

		if (protocol.sol_domains.length > 0) {
			badges.push({ label: 'SNS', count: protocol.sol_domains.length });
		}

		if (protocol.catalog_instruction_count > 0) {
			badges.push({
				label: 'Instruction',
				count: protocol.catalog_instruction_count
			});
		}

		if (protocol.admin_instruction_filters.length > 0) {
			badges.push({
				label: 'Admin instruction',
				count: protocol.admin_instruction_filters.length
			});
		}

		const metricTriggerCount = protocol.amount_triggers.length;
		if (metricTriggerCount > 0) {
			badges.push({ label: 'Amount trigger', count: metricTriggerCount });
		}

		return badges;
	}

	function badgeLabel(badge: MonitorBadge) {
		return `${badge.label}: ${badge.count}`;
	}

	function overviewStats() {
		return [
			{ label: 'Live tip', value: formatCount(data.liveStatus?.summary.last_processed_slot) },
			{ label: 'Parsed tx', value: formatCount(data.liveStatus?.summary.processed_transaction_count) },
			{ label: 'Matched tx', value: formatCount(data.liveStatus?.summary.matched_transaction_count) },
			{ label: 'Indexed events', value: formatCount(data.liveStatus?.summary.total_count) },
			{ label: 'Active watches', value: formatCount(data.telegramSubscriptionCount) },
			{ label: 'Protocols', value: formatCount(data.protocolCount) }
		];
	}

	function watchSignals() {
		return [
			'Program upgrades',
			'Dependency paths',
			'IDL drift',
			'Signer changes',
			'Exit-first alerts'
		];
	}

</script>

<svelte:head>
	<title>FireWatch</title>
</svelte:head>

<div class="page-shell">
	<section class="masthead-shell">
		<article class="panel hero-panel">
			<div class="hero-top">
				<div class="hero-logo-frame" aria-hidden="true">
					<img class="hero-logo-image" src={towerIcon} alt="" width="256" height="256" />
				</div>
				<div class="hero-copy">
					<p class="hero-status">Free for FireWatch members</p>
					<h1>Exit before protocol risk reaches your funds.</h1>
					<p class="lede">
						Dashboards are too late. FireWatch monitors upgrades, governance, IDLs, signer
						changes, and upstream dependencies so a critical alert can become a withdrawal
						signal before contagion spreads.
					</p>
					<div class="hero-actions">
						<a class="hero-cta" href="/join-the-watch">Join the Watch</a>
						<p class="hero-note">
							Watch members help triage alerts and can escalate credible threats to
							<strong>Level Fire</strong>.
						</p>
					</div>
				</div>
			</div>

			<ul class="signal-row" aria-label="FireWatch signals">
				{#each watchSignals() as signal}
					<li>{signal}</li>
				{/each}
			</ul>
		</article>

		<article class="panel setup-panel">
			<div class="setup-head">
				<div>
					<h2>Telegram setup</h2>
					<p class="setup-copy">Add the bot, confirm it is listening, then start the DeFi watch.</p>
				</div>
				<a class="command-link" href="/docs#commands">All commands</a>
			</div>

			<div class="bot-strip">
				<div>
					<span>Bot</span>
					<strong>{botLabel()}</strong>
				</div>
			</div>

			<div class="setup-steps">
				<article class="step-row">
					<span class="step-index">1</span>
					<div>
						<strong>Add</strong>
						<p>Add the bot to the group or channel you want to protect.</p>
					</div>
				</article>

				<article class="step-row">
					<span class="step-index">2</span>
					<div>
						<strong>Confirm</strong>
						<p>Send <code>/help</code> once to confirm the bot is listening.</p>
					</div>
				</article>

				<article class="step-row">
					<span class="step-index">3</span>
					<div>
						<strong>Watch</strong>
						<p>
							Build a focused command on
							<a class="docs-inline-link" href="/join-the-watch#command-builder">Join the Watch</a>.
						</p>
					</div>
				</article>
			</div>
		</article>
	</section>

	<section class="panel stats-strip" aria-label="Overview">
		{#each overviewStats() as stat}
			<article class="stat-card">
				<span>{stat.label}</span>
				<strong>{stat.value}</strong>
			</article>
		{/each}
	</section>

	<section class="panel catalog-panel" id="projects">
		<SectionHeader
			title="Supported protocols"
			description="Open a protocol for watch commands, tracked programs, and recent activity."
			href="/programs"
			hrefLabel="Browse programs"
		/>

		<div class="monitor-legend" aria-label="FireWatch monitor categories">
			{#each monitorLegendBadges() as badge}
				<span class="monitor-badge" title={badgeLabel(badge)} aria-label={badgeLabel(badge)}>
					{badge.label}
					<span class="monitor-count">{badge.count}</span>
				</span>
			{/each}
		</div>

		<div class="protocol-list" role="list">
			{#each data.defiProtocols as protocol}
				<article class="protocol-item" role="listitem">
					<a class="protocol-row" href={protocolPageHref(protocol.slug)}>
						<div class="protocol-main">
							<div class="title-row">
								<ProtocolIdentity slug={protocol.slug} label={protocol.name} />
							</div>
							<div class="protocol-badges" aria-label={`${protocol.name} monitor categories`}>
								{#each protocolMonitorBadges(protocol) as badge}
									<span class="monitor-badge" title={badgeLabel(badge)} aria-label={badgeLabel(badge)}>
										{badge.label}
										<span class="monitor-count">{badge.count}</span>
									</span>
								{/each}
							</div>
						</div>

						<div class="protocol-footer">
							<strong>Open</strong>
						</div>
					</a>
				</article>
			{/each}
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

	.panel,
	.protocol-row {
		background: var(--fw-panel);
		border: 1px solid var(--fw-border);
		border-radius: 10px;
	}

	.panel {
		padding: 1rem;
	}

	.masthead-shell {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(300px, 0.62fr);
		gap: 1rem;
		align-items: start;
	}

	.hero-panel,
	.setup-panel {
		display: grid;
		align-content: start;
		gap: 0.86rem;
	}

	.hero-top {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(8.5rem, 10.5rem);
		align-items: start;
		gap: 1.25rem;
	}

	.hero-copy {
		grid-column: 1;
		grid-row: 1;
		display: grid;
		gap: 0.58rem;
		align-content: start;
		min-width: 0;
	}

	.hero-logo-image {
		display: block;
		width: 82%;
		height: 82%;
		object-fit: contain;
		object-position: center;
	}

	.hero-logo-frame {
		grid-column: 2;
		grid-row: 1;
		display: grid;
		place-items: center;
		width: min(10.5rem, 100%);
		aspect-ratio: 1;
		justify-self: end;
	}

	h1,
	h2,
	strong,
	p {
		margin: 0;
	}

	h1 {
		max-width: 14ch;
		font-size: 2.35rem;
		line-height: 1.03;
		letter-spacing: 0;
	}

	h2 {
		font-size: 1rem;
		line-height: 1.25;
	}

	.hero-status {
		font-size: 0.84rem;
		font-weight: 600;
		color: #8a5e3e;
	}

	.lede,
	.bot-strip span,
	.step-row p,
	.stat-card span {
		color: #62707d;
	}

	.lede {
		max-width: 56ch;
		line-height: 1.5;
		font-size: 0.94rem;
	}

	.hero-actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.8rem;
		margin-top: 0.1rem;
		min-width: 0;
	}

	.hero-cta {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 0.68rem 0.92rem;
		border-radius: 8px;
		background: #bd4a17;
		color: white;
		font-weight: 700;
		text-decoration: none;
	}

	.hero-note {
		flex: 1 1 16rem;
		max-width: 34ch;
		font-size: 0.84rem;
		line-height: 1.45;
		color: #6a7784;
	}

	.signal-row,
	.monitor-legend,
	.protocol-badges,
	.protocol-footer {
		display: flex;
		flex-wrap: wrap;
		gap: 0.55rem;
	}

	.signal-row {
		margin: 0;
		padding: 0.72rem 0 0;
		list-style: none;
		color: #5f6c79;
		font-size: 0.82rem;
		border-top: 1px solid var(--fw-border);
		column-gap: 1rem;
		row-gap: 0.35rem;
	}

	.signal-row li {
		min-width: 0;
	}

	.command-link,
	.docs-inline-link {
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.command-link,
	.docs-inline-link {
		color: #7b5b45;
		font-weight: 600;
		font-size: 0.82rem;
	}

	.docs-inline-link {
		color: #8e694f;
		text-decoration-color: rgba(142, 105, 79, 0.45);
	}

	.setup-head {
		display: flex;
		flex-wrap: wrap;
		align-items: start;
		justify-content: space-between;
		gap: 0.9rem;
	}

	.setup-head > div {
		min-width: 0;
	}

	.setup-copy {
		margin-top: 0.2rem;
		font-size: 0.84rem;
		line-height: 1.45;
		color: #5f6c79;
	}

	.bot-strip {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.8rem;
		padding: 0.72rem 0;
		color: #7d3a18;
		border-top: 1px solid var(--fw-border);
		border-bottom: 1px solid var(--fw-border);
	}

	.bot-strip > div {
		display: grid;
		gap: 0.12rem;
		min-width: 0;
	}

	.bot-strip strong,
	.stat-card strong {
		font-size: 0.96rem;
		overflow-wrap: anywhere;
	}

	.bot-strip span,
	.stat-card span {
		font-size: 0.76rem;
		font-weight: 600;
	}

	.setup-steps {
		display: grid;
	}

	.step-row {
		display: grid;
		grid-template-columns: 24px minmax(0, 1fr);
		gap: 0.7rem;
		padding: 0.75rem 0;
		border-bottom: 1px solid var(--fw-border);
	}

	.step-index {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		border-radius: 6px;
		background: #f3ece3;
		color: #bd4a17;
		font-size: 0.75rem;
		font-weight: 800;
	}

	.step-row p {
		margin-top: 0.14rem;
		line-height: 1.45;
		font-size: 0.88rem;
	}

	.stats-strip {
		display: grid;
		grid-template-columns: repeat(6, minmax(0, 1fr));
		gap: 1px;
		padding: 0;
		overflow: hidden;
		background: var(--fw-border);
	}

	.stat-card {
		display: grid;
		gap: 0.1rem;
		min-width: 0;
		padding: 0.78rem 0.9rem;
		background: var(--fw-panel);
	}

	.catalog-panel {
		display: grid;
		gap: 0.82rem;
		scroll-margin-top: 7.5rem;
	}

	.monitor-legend {
		gap: 0.38rem;
		padding-top: 0.08rem;
	}

	.protocol-list {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
		gap: 0.62rem;
	}

	.protocol-item {
		min-width: 0;
	}

	.protocol-row {
		display: grid;
		grid-template-columns: 1fr;
		gap: 0.58rem;
		align-items: start;
		min-height: 100%;
		padding: 0.76rem 0.82rem;
		color: inherit;
		text-decoration: none;
	}

	.protocol-main {
		display: grid;
		gap: 0.22rem;
		min-width: 0;
	}

	.title-row {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0.5rem;
		min-width: 0;
	}

	.protocol-badges {
		gap: 0.38rem;
		padding-top: 0.1rem;
	}

	.monitor-badge {
		display: inline-flex;
		align-items: center;
		gap: 0.34rem;
		justify-content: center;
		min-height: 1.42rem;
		padding: 0.16rem 0.24rem 0.16rem 0.42rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 6px;
		background: #fbfaf8;
		color: #596775;
		font-size: 0.74rem;
		font-weight: 700;
	}

	.monitor-count {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-width: 1rem;
		height: 1rem;
		padding: 0 0.22rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 999px;
		background: #ffffff;
		color: #17202a;
		font-size: 0.68rem;
		line-height: 1;
	}

	.protocol-footer {
		align-items: center;
		justify-content: flex-start;
		align-self: end;
		padding-top: 0.5rem;
		border-top: 1px solid rgba(23, 32, 42, 0.06);
	}

	.protocol-footer strong {
		font-size: 0.78rem;
		color: #7b5b45;
	}

	@media (max-width: 1100px) {
		.stats-strip {
			grid-template-columns: repeat(3, minmax(0, 1fr));
		}
	}

	@media (max-width: 900px) {
		.masthead-shell {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 720px) {
		.page-shell {
			width: min(100vw - 1.25rem, var(--fw-page-max));
			padding-top: 0.62rem;
			gap: 0.8rem;
		}

		.panel,
		.protocol-row {
			border-radius: 8px;
		}

		.panel {
			padding: 0.86rem;
		}

		.hero-top {
			grid-template-columns: minmax(0, 1fr) minmax(3.8rem, 4.6rem);
			gap: 0.7rem;
		}

		.hero-copy,
		.hero-logo-frame {
			grid-row: 1;
		}

		.hero-logo-frame {
			grid-column: 2;
			justify-self: end;
			max-width: 4.6rem;
		}

		.hero-copy {
			grid-column: 1;
		}

		h1 {
			font-size: 1.86rem;
			line-height: 1.06;
			max-width: none;
		}

		.hero-actions {
			display: grid;
			gap: 0.62rem;
		}

		.hero-cta {
			justify-self: start;
		}

		.signal-row {
			column-gap: 0.75rem;
		}

		.stats-strip {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}

		.stat-card {
			padding: 0.72rem 0.76rem;
		}

		.protocol-list {
			grid-template-columns: 1fr;
		}

		.setup-head {
			flex-direction: column;
		}
	}
</style>
