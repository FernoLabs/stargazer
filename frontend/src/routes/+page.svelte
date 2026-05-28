<script lang="ts">
	import { protocolPageHref } from '$lib/catalog';
	import { watchDefiCommand } from '$lib/bot-commands';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import TelegramCommandBuilder from '$lib/components/TelegramCommandBuilder.svelte';
	import towerIcon from '$lib/assets/firewatch-tower-icon.png';
	import type { DefiProtocolDescriptor } from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	type CopyState = 'idle' | 'copied' | 'error';
	type MonitorBadge = {
		label: string;
		count: number;
	};

	let freeCopyState = $state<CopyState>('idle');

	function botUsername() {
		return data.liveStatus?.telegram_bot?.username ?? null;
	}

	function botHandle() {
		const username = botUsername();
		return username ? `@${username}` : null;
	}

	function botUrl() {
		const username = botUsername();
		return username ? `https://t.me/${username}` : null;
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
		return data.defiProtocols.reduce((count, protocol) => count + protocol.sol_domains.length, 0);
	}

	function overviewStats() {
		return [
			{ label: 'Live tip', value: formatCount(data.liveStatus?.summary.last_processed_slot) },
			{ label: 'Indexed events', value: formatCount(data.liveStatus?.summary.total_count) },
			{ label: 'Matched tx', value: formatCount(data.liveStatus?.summary.matched_transaction_count) },
			{ label: 'Active watches', value: formatCount(data.telegramSubscriptionCount) },
			{ label: 'Protocols', value: formatCount(data.protocolCount) },
			{ label: 'Programs', value: formatCount(data.trackedProgramCount) }
		];
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
			badges.push({ label: 'Instruction', count: protocol.catalog_instruction_count });
		}

		if (protocol.admin_instruction_filters.length > 0) {
			badges.push({ label: 'Admin instruction', count: protocol.admin_instruction_filters.length });
		}

		if (protocol.amount_triggers.length > 0) {
			badges.push({ label: 'Amount trigger', count: protocol.amount_triggers.length });
		}

		return badges;
	}

	function badgeLabel(badge: MonitorBadge) {
		return `${badge.label}: ${badge.count}`;
	}

	function featuredProtocols() {
		return data.defiProtocols.slice(0, 8);
	}

	async function copyFreeCommand() {
		try {
			await navigator.clipboard.writeText(watchDefiCommand());
			freeCopyState = 'copied';
		} catch {
			freeCopyState = 'error';
		}
	}
</script>

<svelte:head>
	<title>FireWatch</title>
</svelte:head>

<div class="page-shell">
	<section class="front-grid" aria-labelledby="front-title">
		<div class="front-copy">
			<img class="tower-mark" src={towerIcon} alt="" width="96" height="96" />
			<div>
				<h1 id="front-title">FireWatch</h1>
				<p>
					Telegram-first Solana monitoring for protocol risk, wallet movement, program
					upgrades, signer changes, IDL drift, and exit-critical alerts.
				</p>
			</div>
		</div>

		<aside class="panel free-setup" id="free-alert" aria-label="Free alert setup">
			<div class="setup-head">
				<h2>Free DeFi alert</h2>
				{#if botUrl()}
					<a href={botUrl()} rel="noreferrer" target="_blank">{botHandle()}</a>
				{/if}
			</div>

			<div class="bot-line">
				<span>Bot</span>
				<strong>{botLabel()}</strong>
			</div>

			<div class="command-line">
				<code>{watchDefiCommand()}</code>
				<button type="button" onclick={copyFreeCommand}>Copy</button>
			</div>

			<ol class="setup-list">
				<li>Add the bot to your Telegram group or channel.</li>
				<li>Send <code>{watchDefiCommand()}</code>.</li>
				<li>Use <code>/list</code> to confirm the rule is active.</li>
			</ol>

			{#if freeCopyState === 'copied'}
				<p class="copy-state success">Copied.</p>
			{:else if freeCopyState === 'error'}
				<p class="copy-state error">Copy failed.</p>
			{/if}
		</aside>
	</section>

	<section class="stats-strip" aria-label="FireWatch stats">
		{#each overviewStats() as stat}
			<article class="stat-cell">
				<span>{stat.label}</span>
				<strong>{stat.value}</strong>
			</article>
		{/each}
	</section>

	<section class="custom-monitor" id="custom-monitor" aria-labelledby="custom-monitor-title">
		<div class="section-copy">
			<h2 id="custom-monitor-title">Build a custom monitor</h2>
			<p>
				Choose a protocol bundle, exact program, or wallet rule. Copy the command into
				Telegram; the bot saves the monitor and returns a pay link when access is missing.
			</p>
		</div>

		<TelegramCommandBuilder
			protocols={data.defiProtocols}
			botUsername={botUsername()}
			defaultPlan="paid"
			defaultMode="protocol"
			showPlanPanel={false}
			heading="Monitor command"
			description="Pick what should be watched and send the generated command to the FireWatch bot."
		/>
	</section>

	<section class="secondary-grid" aria-label="Explore FireWatch">
		<article class="panel explore-panel">
			<div class="section-copy compact">
				<h2>Explore protocols</h2>
				<p>Open a protocol when you need tracked programs, watchable surfaces, and recent activity.</p>
			</div>

			<div class="mini-protocol-list">
				{#each featuredProtocols() as protocol}
					<a class="mini-protocol" href={protocolPageHref(protocol.slug)}>
						<ProtocolIdentity slug={protocol.slug} label={protocol.name} size="sm" />
						<span>{protocol.programs.length} programs</span>
					</a>
				{/each}
			</div>

			<a class="text-link" href="/programs">Browse programs</a>
		</article>

		<article class="panel alert-panel">
			<div class="section-copy compact">
				<h2>Review alerts</h2>
				<p>Use the activity feed after setup to inspect the alert, transaction, protocol tag, and payload.</p>
			</div>

			<div class="alert-actions">
				<a href="/events">Activity feed</a>
				<a href="/docs#commands">Bot commands</a>
				<a href="/pricing#custom-plan">Custom Plan</a>
			</div>

			<div class="monitor-legend" aria-label="Monitor categories">
				{#each monitorLegendBadges() as badge}
					<span class="monitor-badge" title={badgeLabel(badge)} aria-label={badgeLabel(badge)}>
						{badge.label}
						<span class="monitor-count">{badge.count}</span>
					</span>
				{/each}
			</div>
		</article>
	</section>

	<section class="panel company-panel" aria-labelledby="company-title">
		<div class="section-copy">
			<h2 id="company-title">Company response</h2>
			<p>
				For more alert coverage or a security response path, Ferno can review supported
				protocols, give your engineers controlled access, and prepare Smart Account policies
				to exit and redeploy funds after review.
			</p>
		</div>

		<div class="company-actions">
			<a class="primary-link" href="/pricing#pro-response">Review Pro Response</a>
			<a class="text-link" href="mailto:contact@ferno.ag?subject=FireWatch%20company%20response">Contact us</a>
		</div>
	</section>

	<section class="catalog-panel" id="projects" aria-labelledby="supported-title">
		<div class="section-copy">
			<h2 id="supported-title">Supported protocols</h2>
			<p>Protocol pages stay available for teams that want to inspect coverage before building a rule.</p>
		</div>

		<div class="protocol-list" role="list">
			{#each data.defiProtocols as protocol}
				<article class="protocol-item" role="listitem">
					<a class="protocol-row" href={protocolPageHref(protocol.slug)}>
						<div class="protocol-main">
							<ProtocolIdentity slug={protocol.slug} label={protocol.name} />
							<div class="protocol-badges" aria-label={`${protocol.name} monitor categories`}>
								{#each protocolMonitorBadges(protocol) as badge}
									<span class="monitor-badge" title={badgeLabel(badge)} aria-label={badgeLabel(badge)}>
										{badge.label}
										<span class="monitor-count">{badge.count}</span>
									</span>
								{/each}
							</div>
						</div>
						<strong>Open</strong>
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
		padding: 1.05rem 0 2.4rem;
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

	h1,
	h2,
	p,
	ol {
		margin: 0;
	}

	h1 {
		font-size: 2.45rem;
		line-height: 1;
		letter-spacing: 0;
	}

	h2 {
		font-size: 1.08rem;
		line-height: 1.25;
		letter-spacing: 0;
	}

	p,
	li {
		color: #5f6c79;
		line-height: 1.55;
	}

	code {
		font-size: 0.82rem;
		overflow-wrap: anywhere;
	}

	.front-grid {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(310px, 0.5fr);
		gap: 1rem;
		align-items: stretch;
	}

	.front-copy {
		display: grid;
		grid-template-columns: 5.2rem minmax(0, 1fr);
		gap: 1rem;
		align-content: center;
		min-height: 16rem;
		padding: 0.4rem 0;
	}

	.front-copy p {
		max-width: 58ch;
		margin-top: 0.55rem;
		font-size: 1rem;
	}

	.tower-mark {
		width: 5.2rem;
		height: 5.2rem;
		object-fit: contain;
		align-self: start;
	}

	.free-setup {
		display: grid;
		align-content: start;
		gap: 0.8rem;
	}

	.setup-head,
	.company-actions,
	.alert-actions,
	.monitor-legend,
	.protocol-badges {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
	}

	.setup-head {
		align-items: start;
		justify-content: space-between;
	}

	.setup-head a,
	.text-link,
	.secondary-grid a,
	.company-actions a {
		text-decoration: underline;
		text-underline-offset: 0.12em;
		font-size: 0.84rem;
		font-weight: 700;
		color: #6d5a4a;
	}

	.bot-line {
		display: grid;
		gap: 0.12rem;
		padding: 0.72rem 0;
		border-top: 1px solid var(--fw-border);
		border-bottom: 1px solid var(--fw-border);
	}

	.bot-line span,
	.stat-cell span {
		color: #62707d;
		font-size: 0.76rem;
		font-weight: 700;
	}

	.bot-line strong,
	.stat-cell strong {
		overflow-wrap: anywhere;
	}

	.command-line {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 0.55rem;
		align-items: center;
		padding: 0.72rem;
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 8px;
		background: #fbfaf8;
	}

	.command-line code {
		color: #17202a;
		font-weight: 700;
	}

	button,
	.primary-link {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 2.15rem;
		padding: 0 0.78rem;
		border-radius: 8px;
		border: 1px solid #bd4a17;
		background: #bd4a17;
		color: #fff;
		font: inherit;
		font-size: 0.84rem;
		font-weight: 700;
		text-decoration: none;
		cursor: pointer;
	}

	.setup-list {
		display: grid;
		gap: 0.3rem;
		padding-left: 1.1rem;
		font-size: 0.88rem;
	}

	.copy-state {
		font-size: 0.84rem;
		font-weight: 700;
	}

	.copy-state.success {
		color: #216d4d;
	}

	.copy-state.error {
		color: #9c3929;
	}

	.stats-strip {
		display: grid;
		grid-template-columns: repeat(6, minmax(0, 1fr));
		gap: 1px;
		padding: 0;
		overflow: hidden;
		border: 1px solid var(--fw-border);
		border-radius: 10px;
		background: var(--fw-border);
	}

	.stat-cell {
		display: grid;
		gap: 0.1rem;
		min-width: 0;
		padding: 0.78rem 0.9rem;
		background: var(--fw-panel);
	}

	.custom-monitor,
	.catalog-panel {
		display: grid;
		gap: 0.85rem;
		scroll-margin-top: 10rem;
	}

	.custom-monitor {
		padding-top: 0.25rem;
	}

	.section-copy {
		display: grid;
		gap: 0.24rem;
		min-width: 0;
	}

	.section-copy p {
		max-width: 66ch;
		font-size: 0.9rem;
	}

	.section-copy.compact p {
		max-width: none;
	}

	.secondary-grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 1rem;
	}

	.explore-panel,
	.alert-panel,
	.company-panel {
		display: grid;
		align-content: start;
		gap: 0.78rem;
	}

	.mini-protocol-list {
		display: grid;
		gap: 0.42rem;
	}

	.mini-protocol {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.7rem;
		padding: 0.54rem 0;
		border-bottom: 1px solid rgba(23, 32, 42, 0.06);
		text-decoration: none;
	}

	.mini-protocol span {
		color: #62707d;
		font-size: 0.78rem;
		font-weight: 700;
		white-space: nowrap;
	}

	.alert-actions a {
		display: inline-flex;
		align-items: center;
		min-height: 2.1rem;
		padding: 0 0.72rem;
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 8px;
		background: #fbfaf8;
		text-decoration: none;
	}

	.monitor-legend {
		gap: 0.38rem;
		padding-top: 0.08rem;
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

	.company-panel {
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
	}

	.company-actions {
		justify-content: end;
	}

	.protocol-list {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
		gap: 0.62rem;
	}

	.protocol-item,
	.protocol-main {
		min-width: 0;
	}

	.protocol-row {
		display: grid;
		gap: 0.58rem;
		min-height: 100%;
		padding: 0.76rem 0.82rem;
		color: inherit;
		text-decoration: none;
	}

	.protocol-main {
		display: grid;
		gap: 0.25rem;
	}

	.protocol-badges {
		gap: 0.38rem;
		padding-top: 0.1rem;
	}

	.protocol-row > strong {
		padding-top: 0.5rem;
		border-top: 1px solid rgba(23, 32, 42, 0.06);
		color: #7b5b45;
		font-size: 0.78rem;
	}

	@media (max-width: 1100px) {
		.stats-strip {
			grid-template-columns: repeat(3, minmax(0, 1fr));
		}
	}

	@media (max-width: 920px) {
		.front-grid,
		.secondary-grid,
		.company-panel {
			grid-template-columns: 1fr;
		}

		.company-actions {
			justify-content: start;
		}
	}

	@media (max-width: 720px) {
		.page-shell {
			width: min(100vw - 1.25rem, var(--fw-page-max));
			padding-top: 0.72rem;
			gap: 0.85rem;
		}

		.panel,
		.protocol-row,
		.stats-strip {
			border-radius: 8px;
		}

		.front-copy {
			grid-template-columns: minmax(0, 1fr);
			min-height: 0;
			gap: 0.65rem;
			padding-top: 0.2rem;
		}

		.tower-mark {
			width: 4rem;
			height: 4rem;
		}

		h1 {
			font-size: 2rem;
		}

		.stats-strip {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}

		.command-line,
		.mini-protocol {
			grid-template-columns: 1fr;
		}

		.protocol-list {
			grid-template-columns: 1fr;
		}

		.custom-monitor,
		.catalog-panel {
			scroll-margin-top: 12rem;
		}
	}
</style>
