<script lang="ts">
	import { protocolActivityPageHref, protocolPageHref } from '$lib/catalog';
	import { page } from '$app/state';
	import LiveStatusStrip from '$lib/components/LiveStatusStrip.svelte';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import towerIcon from '$lib/assets/firewatch-tower-icon.png';
	import { getLiveStatusContext } from '$lib/live-status-context.svelte';
	import { projectRiskSummary } from '$lib/risk';
	import type { DefiProtocolDescriptor, ProgramRiskSummary } from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	const liveStatusContext = getLiveStatusContext();

	type ProtocolRow = {
		protocol: DefiProtocolDescriptor;
		risk: ProgramRiskSummary;
	};
	type ProtocolAlertRow = {
		slug: string;
		name: string;
		recentAlertCount: number;
		criticalCount: number;
		highCount: number;
		standardCount: number;
		latestSlot: number;
		latestSummary: string;
		latestEventId: string;
	};
	type CoverageMetricCard = {
		label: string;
		value: number | null | undefined;
		href: string;
		tone?: 'critical' | 'high';
		trend?: 'up' | 'down' | 'flat' | 'new';
		detail?: string;
	};
	type QuickstartPlan = 'free' | 'paid' | 'ondemand';
	type FeaturePlan = Exclude<QuickstartPlan, 'free'>;
	type PlanFeatureMap = Record<FeaturePlan, string[]>;

	const PROTOCOL_TEASER_LIMIT = 6;
	const DEFAULT_BOT_USERNAME = 'fernowatch_bot';
	const homeNav = [
		{ href: '/', label: 'Watch' },
		{ href: '/protocols', label: 'Protocols' },
		{ href: '/events', label: 'Activity' },
		{ href: '/programs', label: 'Programs' },
		{ href: '/about', label: 'About' }
	];

	const QUICKSTART_PLAN_FEATURES: PlanFeatureMap = {
		paid: [
			'Multi-protocol custom watchlists',
			'Auto-track protocols based on wallet interactions',
			'Wallet, program, .sol, and amount-trigger rules'
		],
		ondemand: [
			'Withdraw funds during critical alerts',
			'Redeploy once the alert is over',
			'Dedicated low-latency Ferno infra'
		]
	};

	let copyToast = $state<{ message: string; error: boolean } | null>(null);
	let copyToastTimer: number | null = null;
	let sortedProtocolRows = $derived.by(() =>
		data.defiProtocols
			.map((protocol) => ({ protocol, risk: projectRiskSummary(protocol) }))
			.sort(
				(left, right) =>
					right.risk.critical_total - left.risk.critical_total ||
					right.risk.unreviewed_critical - left.risk.unreviewed_critical ||
					left.protocol.name.localeCompare(right.protocol.name)
			)
	);
	let coverageMetricCards = $derived.by<CoverageMetricCard[]>(() => [
		{ label: 'Tracked protocols', value: data.protocolCount, href: '/protocols' },
		{ label: 'Tracked programs', value: data.trackedProgramCount, href: '/programs' },
		{ label: 'Indexed events', value: data.liveStatus?.summary.total_count, href: '/events' },
		{
			label: `Events last ${data.homeActivityWindow.days}d`,
			value: data.homeActivityWindow.current,
			href: '/events',
			trend: activityWindowTrend(),
			detail: activityWindowDetail()
		},
		{ label: 'Subscriptions', value: data.telegramSubscriptionCount, href: '#get-started' },
		{
			label: 'Program versions',
			value: data.homeProgramVersionStats.versions,
			href: '/programs'
		},
		{
			label: 'Critical alerts',
			value: data.homeAlertCounts.critical,
			href: '/events?severity=critical',
			tone: 'critical'
		},
		{
			label: 'High alerts',
			value: data.homeAlertCounts.high,
			href: '/events?severity=high',
			tone: 'high'
		}
	]);

	function botUsername() {
		const username = liveStatusContext.liveStatus?.telegram_bot?.username?.trim();
		return username || DEFAULT_BOT_USERNAME;
	}

	function botHandle() {
		return `@${botUsername()}`;
	}

	function addBotUrl() {
		return `https://t.me/${botUsername()}?startgroup=setup`;
	}

	function botChatUrl() {
		return `https://t.me/${botUsername()}`;
	}

	function formatCount(value: number | null | undefined) {
		return new Intl.NumberFormat('en-US').format(value ?? 0);
	}

	function activityWindowTrend(): CoverageMetricCard['trend'] {
		const change = data.homeActivityWindow.percentChange;
		if (change == null) return 'new';
		if (change > 0) return 'up';
		if (change < 0) return 'down';
		return 'flat';
	}

	function activityWindowDetail() {
		const { days, percentChange } = data.homeActivityWindow;
		if (percentChange == null) return `New vs previous ${days}d`;
		if (percentChange === 0) return `Flat vs previous ${days}d`;
		const sign = percentChange > 0 ? '+' : '';
		return `${sign}${percentChange}% vs previous ${days}d`;
	}

	function coverageLabel(protocol: DefiProtocolDescriptor) {
		if (protocol.coverage === 'deep_decode') return 'Instructions parsed';
		if (protocol.coverage === 'admin_signals') return 'Admin signals';
		return 'Address coverage';
	}

	function protocolBySlug(slug: string) {
		return data.defiProtocols.find((protocol) => protocol.slug === slug) ?? null;
	}

	function protocolRisk(slug: string) {
		return sortedProtocolRows.find((row) => row.protocol.slug === slug)?.risk ?? null;
	}

	function fallbackAlertRows(): ProtocolAlertRow[] {
		return sortedProtocolRows
			.filter((row) => row.risk.critical_total > 0)
			.slice(0, PROTOCOL_TEASER_LIMIT)
			.map((row) => ({
				slug: row.protocol.slug,
				name: row.protocol.name,
				recentAlertCount: row.risk.critical_total,
				criticalCount: row.risk.critical_total,
				highCount: 0,
				standardCount: 0,
				latestSlot: 0,
				latestSummary: 'Critical proof attached to this project.',
				latestEventId: ''
			}));
	}

	function protocolAlertRows() {
		return data.homeProtocolAlertRows.length > 0 ? data.homeProtocolAlertRows : fallbackAlertRows();
	}

	function activityHref(protocol: DefiProtocolDescriptor) {
		return protocolActivityPageHref(protocol.slug);
	}

	function protocolActivityHref(slug: string) {
		const protocol = protocolBySlug(slug);
		return protocol ? activityHref(protocol) : `/events?protocol=${encodeURIComponent(slug)}`;
	}

	function protocolSeverityHref(slug: string, severity: 'critical' | 'high' | 'standard') {
		const search = new URLSearchParams({ protocol: slug, severity });
		return `/events?${search.toString()}`;
	}

	function eventHref(eventId: string, slug: string) {
		return eventId ? `/events/${encodeURIComponent(eventId)}` : protocolActivityHref(slug);
	}

	function customPlanToken() {
		return data.customPlanPricing?.token ?? 'USDC';
	}

	function customPlanMonthlyLabel() {
		const monthly = data.customPlanPricing?.monthly ?? '0.42';
		return `${monthly} ${customPlanToken()} / month`;
	}

	function isActive(pathname: string, href: string) {
		if (href === '/') return pathname === '/';
		return pathname === href || pathname.startsWith(`${href}/`);
	}

	function quickstartPlanFeatures(plan: FeaturePlan) {
		return QUICKSTART_PLAN_FEATURES[plan];
	}

	function watchCommand() {
		return '/watch';
	}

	function configCommand() {
		return '/config';
	}

	function copyCommandWithSelectionFallback(command: string) {
		const textarea = document.createElement('textarea');
		textarea.value = command;
		textarea.setAttribute('readonly', '');
		textarea.style.position = 'fixed';
		textarea.style.top = '0';
		textarea.style.left = '0';
		textarea.style.width = '1px';
		textarea.style.height = '1px';
		textarea.style.opacity = '0';
		textarea.style.pointerEvents = 'none';
		document.body.appendChild(textarea);
		textarea.focus({ preventScroll: true });
		textarea.select();
		textarea.setSelectionRange(0, command.length);
		const copied = document.execCommand('copy');
		textarea.remove();
		if (!copied) throw new Error('Copy command failed');
	}

	async function copyCommand(command: string) {
		try {
			try {
				if (typeof navigator === 'undefined' || !navigator.clipboard) {
					throw new Error('Clipboard is unavailable');
				}

				await navigator.clipboard.writeText(command);
			} catch {
				copyCommandWithSelectionFallback(command);
			}

			copyToast = { message: `Copied ${command} to clipboard`, error: false };
		} catch {
			copyToast = { message: `Could not copy ${command}`, error: true };
		}

		if (copyToastTimer != null) {
			window.clearTimeout(copyToastTimer);
		}

		copyToastTimer = window.setTimeout(() => {
			copyToast = null;
			copyToastTimer = null;
		}, 1800);
	}
</script>

<svelte:head>
	<title>FireWatch</title>
</svelte:head>

{#if copyToast}
	<div class="copy-toast" class:error={copyToast.error} role="status">
		{copyToast.message}
	</div>
{/if}

<div class="page-shell">
	<section class="panel hero" aria-labelledby="home-title">
		<div class="hero-intro" id="home-hero">
			<div class="hero-main">
				<img class="tower-mark" src={towerIcon} alt="" width="80" height="80" />
				<div>
					<h1 id="home-title">FireWatch</h1>
					<p class="lede">
						Solana protocol monitoring for Telegram groups. Start free with shared DeFi coverage,
						then move to paid custom bot rules when your team needs protocol-specific routing.
					</p>
				</div>
				<a class="home-quickstart-cta" href={addBotUrl()} rel="noreferrer" target="_blank">
					Add {botHandle()}
				</a>
			</div>

			<div class="hero-utility">
				<nav class="hero-nav" aria-label="Primary">
					{#each homeNav as item (item.href)}
						<a class:active={isActive(page.url.pathname, item.href)} href={item.href}>
							{item.label}
						</a>
					{/each}
				</nav>

				<div class="hero-live-strip" aria-label="Live status">
					<LiveStatusStrip status={liveStatusContext} />
				</div>
			</div>
		</div>
	</section>

	<aside class="setup-panel" id="get-started">
		<div class="panel-head">
			<h2>Quick Start</h2>
		</div>

		<div class="plan-accordion" aria-label="Quickstart plans">
			<article class="plan-line">
				<div class="plan-toggle">
					<span>Free DeFi watcher</span>
					<strong>Free</strong>
				</div>
				<div class="plan-body">
					<p>Default DeFi monitoring across {formatCount(data.protocolCount)} supported protocols.</p>
				</div>
				<div class="plan-footer command-footer">
					<span>Send</span>
					<button
						class="command-chip"
						type="button"
						aria-label={`Copy ${watchCommand()} command`}
						onclick={() => copyCommand(watchCommand())}
					>
						<code>{watchCommand()}</code>
					</button>
					<span>to</span>
					<a class="bot-action" href={botChatUrl()} rel="noreferrer" target="_blank">
						{botHandle()}
					</a>
				</div>
			</article>

			<article class="plan-line member-plan">
				<div class="plan-toggle">
					<span>Custom watcher</span>
					<div class="price-stack">
						<strong>{customPlanMonthlyLabel()}</strong>
						<a class="member-badge" href="https://x.com/ferno_ag" rel="noreferrer" target="_blank">
							Free for Ferno members
						</a>
					</div>
				</div>
				<div class="plan-body">
					<ul>
						{#each quickstartPlanFeatures('paid') as feature (feature)}
							<li>{feature}</li>
						{/each}
					</ul>
				</div>
				<div class="plan-footer command-footer">
					<span>Send</span>
					<button
						class="command-chip"
						type="button"
						aria-label={`Copy ${configCommand()} command`}
						onclick={() => copyCommand(configCommand())}
					>
						<code>{configCommand()}</code>
					</button>
					<span>to</span>
					<a class="bot-action" href={botChatUrl()} rel="noreferrer" target="_blank">
						{botHandle()}
					</a>
				</div>
			</article>

			<article class="plan-line">
				<div class="plan-toggle">
					<span>Smart Wallet response</span>
					<strong>Contact us</strong>
				</div>
				<div class="plan-body">
					<ul>
						{#each quickstartPlanFeatures('ondemand') as feature (feature)}
							<li>{feature}</li>
						{/each}
					</ul>
				</div>
				<div class="plan-footer">
					<span>Contact us for more info.</span>
				</div>
			</article>
		</div>
	</aside>

	<section class="panel stats-panel" aria-label="FireWatch coverage stats">
		<div class="section-title">
			<div>
				<h2>Watch Overview</h2>
				<p>Coverage, activity, subscriptions, and alert shortcuts. Each number opens the matching view.</p>
			</div>
			<a href="/events">Open activity</a>
		</div>

		<div class="stats-grid">
			{#each coverageMetricCards as card (card.label)}
				<a
					class="stat-card"
					class:critical={card.tone === 'critical'}
					class:high={card.tone === 'high'}
					class:trend-up={card.trend === 'up'}
					class:trend-down={card.trend === 'down'}
					href={card.href}
				>
					<strong>{formatCount(card.value)}</strong>
					<span>{card.label}</span>
					{#if card.detail}
						<span class="metric-detail">{card.detail}</span>
					{/if}
				</a>
			{/each}
		</div>
	</section>

	<section class="panel protocol-panel" id="protocols" aria-labelledby="protocol-title">
		<div class="section-title">
			<div>
				<h2 id="protocol-title">Tracked Protocols</h2>
				<p>
					Protocols with the most alerts in the latest DeFi activity sample.
				</p>
			</div>
			<a href="/protocols">Open protocols</a>
		</div>

		<div class="protocol-table" role="list">
			{#each protocolAlertRows() as row (row.slug)}
				{@const protocol = protocolBySlug(row.slug)}
				{@const risk = protocolRisk(row.slug)}
				<article class="protocol-row" role="listitem">
					<div class="protocol-main">
						<a class="protocol-name" href={protocol ? protocolPageHref(row.slug) : protocolActivityHref(row.slug)}>
							<ProtocolIdentity slug={row.slug} label={row.name} />
						</a>
						<span class="coverage">{protocol ? coverageLabel(protocol) : 'DeFi alerts'}</span>
					</div>

					<div class="alert-cell">
						<div class="alert-strip" aria-label={`${row.name} alert counts`}>
							<a href={protocolActivityHref(row.slug)}>
								Alerts <strong>{formatCount(row.recentAlertCount)}</strong>
							</a>
							<a class="critical-link" href={protocolSeverityHref(row.slug, 'critical')}>
								Critical <strong>{formatCount(row.criticalCount)}</strong>
							</a>
							<a class="high-link" href={protocolSeverityHref(row.slug, 'high')}>
								High <strong>{formatCount(row.highCount)}</strong>
							</a>
							<a href={protocolSeverityHref(row.slug, 'standard')}>
								Normal <strong>{formatCount(row.standardCount)}</strong>
							</a>
						</div>
						<a class="latest-alert" href={eventHref(row.latestEventId, row.slug)}>
							{row.latestSummary}
						</a>
					</div>

					<div class="badge-row">
						{#if risk}
							<span class={`risk-grade grade-${risk.grade.toLowerCase()}`}>Grade {risk.grade}</span>
							<span>Score {risk.score}</span>
						{/if}
						{#if protocol}
							<span>Programs {formatCount(protocol.programs.length)}</span>
						{/if}
						{#if row.latestSlot > 0}
							<span>Latest slot {formatCount(row.latestSlot)}</span>
						{/if}
					</div>

					<div class="row-actions">
						<a href={protocolActivityHref(row.slug)}>Activity</a>
						{#if protocol}
							<a href={protocolPageHref(row.slug)}>Open protocol</a>
						{/if}
					</div>
				</article>
			{/each}
		</div>

		{#if protocolAlertRows().length === 0}
			<p class="protocol-note">No alert-heavy protocols are available yet.</p>
		{:else}
			<p class="protocol-note">
				Showing up to {PROTOCOL_TEASER_LIMIT} protocols from recent DeFi alerts.
				<a href="/events?scope=defi">All DeFi activity</a>
			</p>
		{/if}
	</section>
</div>

<style>
	.page-shell {
		width: 100%;
		margin: 0 auto;
		padding: 0 0 2.4rem;
		display: grid;
		gap: 1rem;
	}

	.panel {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		background: var(--fw-panel);
		border: 1px solid var(--fw-border);
		border-radius: 10px;
		padding: 1rem;
	}

	h1,
	h2,
	p {
		margin: 0;
	}

	h1 {
		font-size: 2.2rem;
		line-height: 1.02;
		letter-spacing: 0;
	}

	h2 {
		font-size: 1.03rem;
		line-height: 1.25;
	}

	p {
		color: #5f6c79;
		line-height: 1.55;
	}

	a {
		color: #5d4c3e;
		font-weight: 750;
		text-underline-offset: 0.14em;
	}

	.copy-toast {
		position: fixed;
		right: max(1rem, calc((100vw - var(--fw-page-max)) / 2));
		bottom: 1rem;
		z-index: 60;
		max-width: min(18rem, calc(100vw - 2rem));
		padding: 0.62rem 0.76rem;
		border: 1px solid rgba(33, 109, 77, 0.18);
		border-radius: 8px;
		background: #f2fbf6;
		color: #216d4d;
		box-shadow: 0 14px 34px rgba(23, 32, 42, 0.12);
		font-size: 0.8rem;
		font-weight: 760;
	}

	.copy-toast.error {
		border-color: rgba(156, 57, 41, 0.18);
		background: #fff4f0;
		color: #9c3929;
	}

	code {
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
		font-size: 0.82rem;
		overflow-wrap: anywhere;
	}

	.hero {
		width: 100%;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: 1fr;
		gap: 0;
		align-items: stretch;
		background: #f8f5f0;
		border: 0;
		border-bottom: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 0;
	}

	.hero-intro {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		display: grid;
		gap: 0;
	}

	.home-quickstart-cta {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 2.02rem;
		padding: 0 0.74rem;
		border-radius: 8px;
		border: 1px solid #bd4a17;
		background: #bd4a17;
		color: #fff;
		font-size: 0.78rem;
		font-weight: 780;
		text-decoration: none;
		white-space: nowrap;
		position: fixed;
		top: 1.1rem;
		right: max(1rem, calc((100vw - var(--fw-page-max)) / 2));
		z-index: 25;
	}

	.hero-utility {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		min-width: 0;
		padding: 0.72rem 0 0.62rem;
		border-top: 1px solid rgba(23, 32, 42, 0.08);
	}

	.hero-nav {
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem;
	}

	.hero-nav a {
		padding: 0.42rem 0 0.5rem;
		border-bottom: 2px solid transparent;
		text-decoration: none;
		font-weight: 700;
		font-size: 0.86rem;
		color: #556473;
	}

	.hero-nav a.active {
		color: var(--fw-accent);
		border-bottom-color: var(--fw-accent);
	}

	.hero-live-strip {
		display: flex;
		flex-wrap: nowrap;
		gap: 0;
		min-width: 0;
		justify-content: flex-end;
	}

	.hero-main {
		display: grid;
		grid-template-columns: 6.2rem minmax(0, 1fr);
		gap: 1.25rem;
		align-items: center;
		padding: 2.15rem 0 2.45rem;
	}

	.lede {
		margin-top: 0.52rem;
		max-width: 62ch;
		font-size: 0.98rem;
	}

	.tower-mark {
		width: 6.2rem;
		height: 6.2rem;
		object-fit: contain;
	}

	.setup-panel,
	.protocol-panel {
		display: grid;
		gap: 0.82rem;
	}

	.setup-panel {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem;
		background: var(--fw-panel);
		border: 1px solid var(--fw-border);
		border-radius: 10px;
	}

	.panel-head,
	.section-title {
		display: flex;
		align-items: start;
		justify-content: space-between;
		gap: 1rem;
	}

	.panel-head {
		align-items: center;
	}

	.panel-head h2 {
		margin: 0;
	}

	.section-title p {
		margin-top: 0.24rem;
	}

	.plan-accordion {
		display: grid;
		gap: 0.5rem;
	}

	.plan-line {
		display: flex;
		flex-direction: column;
		margin: 0;
		padding: 0;
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 8px;
		background: #fbfaf8;
	}

	.member-plan {
		border-color: rgba(189, 74, 23, 0.28);
		background: #fff8f3;
	}

	.plan-body {
		display: block;
	}

	.plan-toggle {
		display: grid;
		grid-template-columns: 1fr auto;
		align-items: center;
		width: 100%;
		gap: 0.6rem;
		padding: 0.68rem 0.75rem;
		min-height: 0;
		border: 0;
		border-radius: 8px;
		background: transparent;
		color: inherit;
		text-align: left;
		cursor: default;
	}

	.plan-toggle span {
		color: #17202a;
		font-size: 0.84rem;
		font-weight: 700;
	}

	.plan-toggle strong {
		color: #7d3a18;
		font-size: 0.78rem;
		font-weight: 800;
		justify-self: end;
		text-align: right;
		padding-left: 0.6rem;
		white-space: nowrap;
	}

	.price-stack {
		display: grid;
		justify-items: end;
		gap: 0.22rem;
		min-width: max-content;
	}

	.plan-toggle .member-badge {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 1.18rem;
		padding: 0 0.4rem;
		border-radius: 999px;
		background: rgba(189, 74, 23, 0.12);
		color: #8d421a;
		font-size: 0.64rem;
		font-weight: 800;
		line-height: 1;
		text-decoration: none;
		white-space: nowrap;
	}

	.plan-line p {
		margin: 0;
		padding: 0 0.75rem 0.54rem;
		font-size: 0.82rem;
		color: #5f6c79;
	}

	.plan-line ul {
		margin: 0;
		padding: 0 0.75rem 0.58rem;
		list-style: none;
		display: grid;
		gap: 0.25rem;
	}

	.plan-line li {
		position: relative;
		padding-left: 0.88rem;
		font-size: 0.77rem;
		line-height: 1.35;
		color: #5d6d7a;
	}

	.plan-line li::before {
		content: '•';
		position: absolute;
		left: 0;
		top: 0;
		color: #bd4a17;
	}

	.plan-footer {
		display: flex;
		flex-wrap: wrap;
		gap: 0.48rem;
		align-items: center;
		justify-content: space-between;
		padding: 0 0.75rem 0.72rem;
		border-top: 1px solid rgba(23, 32, 42, 0.08);
		margin-top: auto;
		padding-top: 0.5rem;
	}

	.plan-footer span {
		font-size: 0.78rem;
		font-weight: 700;
		color: #5f6c79;
	}

	.command-footer {
		justify-content: flex-start;
		gap: 0.36rem;
	}

	.command-footer .bot-action {
		margin-left: auto;
	}

	.bot-action {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 1.72rem;
		padding: 0 0.58rem;
		border-radius: 7px;
		border: 1px solid #bd4a17;
		background: #bd4a17;
		color: #fff;
		font-size: 0.76rem;
		font-weight: 780;
		line-height: 1;
		text-decoration: none;
		white-space: nowrap;
	}

	button {
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
		font-weight: 750;
		cursor: pointer;
	}

	.command-chip {
		min-height: 1.86rem;
		padding: 0 0.64rem;
		border: 1px solid rgba(189, 74, 23, 0.28);
		border-radius: 7px;
		background: #fff;
		color: #7d3a18;
		box-shadow: inset 0 0 0 1px rgba(189, 74, 23, 0.06);
		font-size: 0.8rem;
		font-weight: 800;
		white-space: nowrap;
	}

	.command-chip:hover {
		background: rgba(255, 123, 54, 0.11);
	}

	.command-chip:focus-visible {
		outline: 2px solid rgba(189, 74, 23, 0.36);
		outline-offset: 2px;
	}

	.command-chip code {
		color: inherit;
		font-size: 0.84rem;
		font-weight: 800;
	}

	.secondary-link {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 2.16rem;
		padding: 0 0.84rem;
		border-radius: 8px;
		font-size: 0.8rem;
		font-weight: 700;
		text-decoration: none;
		white-space: nowrap;
	}

	.primary-link {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 2.02rem;
		padding: 0 0.74rem;
		border-radius: 8px;
		font-size: 0.78rem;
		font-weight: 780;
		border: 1px solid #bd4a17;
		background: #bd4a17;
		color: #fff;
		text-decoration: none;
		white-space: nowrap;
	}

	.secondary-link {
		border: 1px solid rgba(23, 32, 42, 0.12);
		background: #fff;
		color: #556473;
	}

	.stats-panel {
		display: grid;
		gap: 0.8rem;
	}

	.stats-grid {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: 0.55rem;
	}

	.stat-card {
		display: grid;
		gap: 0.16rem;
		padding: 0.72rem;
		border-radius: 8px;
		background: #fbfaf8;
		border: 1px solid rgba(23, 32, 42, 0.08);
		color: inherit;
		text-decoration: none;
	}

	.stat-card:hover {
		border-color: rgba(189, 74, 23, 0.22);
		background: #fff8f3;
	}

	.stat-card.critical strong {
		color: #9c3929;
	}

	.stat-card.high strong {
		color: #8a5e1e;
	}

	.stat-card.trend-up .metric-detail {
		color: #8a5e1e;
	}

	.stat-card.trend-down .metric-detail {
		color: #216d4d;
	}

	.stat-card strong {
		font-size: 1rem;
		color: #17202a;
	}

	.stat-card > span {
		font-size: 0.78rem;
		font-weight: 700;
		color: #62707d;
	}

	.stat-card .metric-detail {
		font-size: 0.72rem;
		line-height: 1.25;
		color: #73808c;
	}

	.protocol-table {
		display: grid;
		border: 1px solid var(--fw-border);
		border-radius: 8px;
		overflow: hidden;
	}

	.protocol-row {
		display: grid;
		grid-template-columns: minmax(190px, 0.62fr) minmax(290px, 1fr) minmax(180px, 0.62fr) auto;
		gap: 0.8rem;
		align-items: center;
		padding: 0.78rem;
		background: #fff;
		border-bottom: 1px solid var(--fw-border);
	}

	.protocol-row:last-child {
		border-bottom: 0;
	}

	.protocol-main {
		display: grid;
		gap: 0.2rem;
	}

	.protocol-name {
		text-decoration: none;
	}

	.coverage {
		font-size: 0.76rem;
		font-weight: 700;
		color: #62707d;
	}

	.alert-cell {
		display: grid;
		gap: 0.28rem;
		min-width: 0;
	}

	.alert-strip {
		display: flex;
		flex-wrap: wrap;
		gap: 0.34rem 0.5rem;
		color: #596775;
	}

	.alert-strip a,
	.latest-alert {
		text-decoration: none;
	}

	.alert-strip a {
		color: #596775;
	}

	.alert-strip a {
		font-size: 0.74rem;
		font-weight: 700;
	}

	.alert-strip strong {
		font-size: 0.78rem;
		color: #17202a;
	}

	.alert-strip a:hover,
	.latest-alert:hover {
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.alert-strip .critical-link strong {
		color: #9c3929;
	}

	.alert-strip .high-link strong {
		color: #8a5e1e;
	}

	.latest-alert {
		min-width: 0;
		overflow: hidden;
		color: #5f6c79;
		font-size: 0.78rem;
		font-weight: 650;
		line-height: 1.35;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.risk-grade {
		color: #216d4d;
	}

	.risk-grade.grade-c,
	.risk-grade.grade-d {
		color: #8a5e1e;
	}

	.risk-grade.grade-f {
		color: #9c3929;
	}

	.badge-row,
	.row-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.42rem;
	}

	.badge-row span {
		color: #62707d;
		font-size: 0.76rem;
		font-weight: 700;
	}

	.row-actions a {
		font-size: 0.8rem;
	}

	.protocol-note {
		font-size: 0.82rem;
		color: #62707d;
	}

	.protocol-note a {
		margin-left: 0.3rem;
	}

	@media (max-width: 980px) {
		.stats-grid {
			grid-template-columns: repeat(3, minmax(0, 1fr));
		}

		.protocol-row {
			grid-template-columns: 1fr;
			gap: 0.56rem;
		}
	}

	@media (min-width: 1024px) {
		.plan-accordion {
			grid-template-columns: repeat(3, minmax(0, 1fr));
			align-items: stretch;
		}
	}

	@media (max-width: 720px) {
		.page-shell {
			width: 100%;
			padding-top: 0;
		}

		.hero-main {
			grid-template-columns: 5.2rem minmax(0, 1fr);
			min-height: 0;
			padding: 4.35rem 0 1.05rem;
			gap: 0.9rem;
		}

		.home-quickstart-cta {
			top: 0.5rem;
			right: max(0.75rem, calc((100vw - var(--fw-page-max)) / 2));
			max-width: min(13rem, calc(100vw - 1.5rem));
			min-height: 1.95rem;
			font-size: 0.76rem;
		}

		.hero-utility {
			display: grid;
			gap: 0.42rem;
		}

		.hero-nav {
			flex-wrap: nowrap;
			overflow-x: auto;
			scrollbar-width: none;
		}

		.hero-nav a {
			flex: 0 0 auto;
		}

		.hero-live-strip {
			overflow-x: auto;
			justify-content: flex-start;
			scrollbar-width: none;
			padding-bottom: 0.02rem;
		}

		.tower-mark {
			width: 5.2rem;
			height: 5.2rem;
		}

		.stats-grid {
			grid-template-columns: repeat(2, minmax(0, 1fr));
			gap: 0.46rem;
		}

		.stat-card {
			padding: 0.6rem;
		}

		.stat-card strong {
			font-size: 0.92rem;
		}

		.stat-card span {
			font-size: 0.72rem;
			line-height: 1.25;
		}

		.latest-alert {
			white-space: normal;
		}

		.plan-toggle {
			grid-template-columns: minmax(0, 1fr) auto;
			gap: 0.6rem;
		}

		.plan-footer {
			display: grid;
			grid-template-columns: 1fr auto;
			align-items: center;
			gap: 0.52rem;
		}

		.plan-footer.command-footer {
			grid-template-columns: auto auto auto minmax(0, 1fr);
			gap: 0.34rem;
		}

		.command-footer .bot-action {
			justify-self: end;
			margin-left: 0;
		}

		.section-title {
			display: grid;
		}
	}
</style>
