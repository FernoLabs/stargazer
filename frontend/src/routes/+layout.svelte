<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import favicon from '$lib/assets/firewatch-tower-icon-favicon.png';
	import { WalletProvider } from '@fernolab/wallet-adapter-svelte';
	import BrandLockup from '$lib/components/BrandLockup.svelte';
	import LiveStatusStrip from '$lib/components/LiveStatusStrip.svelte';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import { createLiveStatusContext } from '$lib/live-status-context.svelte';
	import type { LiveStatusResponse } from '$lib/types';

	let { data, children } = $props();
	type PreviewMeta = {
		imageUrl?: string;
		alertLabel?: string;
	};

	const SLOT_TICK_MS = 1000;
	const LIVE_REFRESH_MS = 10_000;
	const INITIAL_REFRESH_DELAY_MS = 6500;
	const DEFAULT_BOT_USERNAME = 'fernowatch_bot';
	const HOME_HEADER_REVEAL_LEAD_PX = 36;

	const primaryNav = [
		{ href: '/', label: 'Watch' },
		{ href: '/protocols', label: 'Protocols' },
		{ href: '/events', label: 'Activity' },
		{ href: '/incidents', label: 'Incidents' },
		{ href: '/programs', label: 'Programs' },
		{ href: '/about', label: 'About' }
	];

	const liveStatusContext = createLiveStatusContext(
		() => data.liveStatus ?? null,
		() => data.telegramSubscriptionCount
	);
	let heroPassed = $state(false);
	let headerMultiline = $state(false);
	let navAndMetricStacked = $state(false);
	let topbarRowRef: HTMLElement | null = null;
	let brandRef: HTMLElement | null = null;
	let primaryNavRef: HTMLElement | null = null;
	let liveStripRef: HTMLElement | null = null;
	let topbarActionsRef: HTMLElement | null = null;
	let heroScrollRaf = $state<number | null>(null);
	let headerLayoutRaf = $state<number | null>(null);
	let headerLayoutResizeObserver: ResizeObserver | null = null;

	$effect(() => {
		liveStatusContext.liveStatus = data.liveStatus ?? null;
		liveStatusContext.telegramSubscriptionCount = data.telegramSubscriptionCount;
	});

	function isActive(pathname: string, href: string) {
		if (href === '/') return pathname === '/';
		return pathname === href || pathname.startsWith(`${href}/`);
	}

	function defaultPreviewImageUrl() {
		return `${page.url.origin}/brand/firewatch-preview-standard.png`;
	}

	function previewMeta() {
		return page.data.preview as PreviewMeta | undefined;
	}

	function previewImageUrl() {
		return previewMeta()?.imageUrl ?? defaultPreviewImageUrl();
	}

	function previewImageAlt() {
		return previewMeta()?.alertLabel ? `${previewMeta()?.alertLabel} from FireWatch` : 'FireWatch logo';
	}

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

	const isHomePage = $derived(page.url.pathname === '/');
	const showTopbar = $derived.by(() => !isHomePage || heroPassed);
	const showHeaderCta = $derived.by(() => showTopbar);

	function recalculateHomeHeaderReveal() {
		if (!isHomePage) {
			heroPassed = true;
			return;
		}

		const hero = document.getElementById('home-hero');
		if (!hero) {
			heroPassed = false;
			return;
		}

		const heroUtility = hero.querySelector('.hero-utility') as HTMLElement | null;
		if (heroUtility) {
			const utilityTop = heroUtility.getBoundingClientRect().top + window.scrollY;
			heroPassed = window.scrollY >= Math.max(0, utilityTop - HOME_HEADER_REVEAL_LEAD_PX);
			return;
		}

		const heroBottom = hero.getBoundingClientRect().bottom + window.scrollY;
		heroPassed = window.scrollY >= Math.max(0, heroBottom - 1);
	}

	function scheduleHomeHeaderRevealRefresh() {
		if (typeof window === 'undefined') return;
		if (heroScrollRaf) {
			window.cancelAnimationFrame(heroScrollRaf);
		}

		heroScrollRaf = window.requestAnimationFrame(() => {
			recalculateHomeHeaderReveal();
			heroScrollRaf = null;
		});
	}

	$effect(() => {
		if (typeof document === 'undefined' || typeof window === 'undefined') return;

		if (heroScrollRaf) {
			window.cancelAnimationFrame(heroScrollRaf);
			heroScrollRaf = null;
		}

		if (!isHomePage) {
			heroPassed = true;
			return;
		}

		const hero = document.getElementById('home-hero') as HTMLElement | null;
		if (!hero) {
			heroPassed = false;
			return;
		}

		recalculateHomeHeaderReveal();
		window.addEventListener('scroll', scheduleHomeHeaderRevealRefresh, { passive: true });
		window.addEventListener('resize', scheduleHomeHeaderRevealRefresh);

		window.requestAnimationFrame(() => recalculateHomeHeaderReveal());

		return () => {
			window.removeEventListener('scroll', scheduleHomeHeaderRevealRefresh);
			window.removeEventListener('resize', scheduleHomeHeaderRevealRefresh);
			if (heroScrollRaf) {
				window.cancelAnimationFrame(heroScrollRaf);
				heroScrollRaf = null;
			}
		};
	});

	function recalculateHeaderLayout() {
		if (!topbarRowRef || !brandRef || !primaryNavRef || !liveStripRef || !topbarActionsRef) {
			headerMultiline = false;
			navAndMetricStacked = false;
			return;
		}

		const rowWidth = topbarRowRef.clientWidth;
		if (!rowWidth) {
			headerMultiline = false;
			navAndMetricStacked = false;
			return;
		}

		const rowStyle = getComputedStyle(topbarRowRef);
		const navStyle = getComputedStyle(primaryNavRef);
		const rowGap = Number.parseFloat(rowStyle.columnGap || rowStyle.gap || '0') || 0;
		const navGap = Number.parseFloat(navStyle.columnGap || navStyle.gap || '0') || 0;
		const navLinks = Array.from(primaryNavRef.querySelectorAll('a'));
		const navWidth =
			navLinks.reduce((sum, link) => sum + link.getBoundingClientRect().width, 0) +
			navGap * Math.max(0, navLinks.length - 1);
		const brandWidth = brandRef.getBoundingClientRect().width;
		const stripWidth = liveStripRef.scrollWidth;
		const actionsVisible = getComputedStyle(topbarActionsRef).display !== 'none';
		const actionsWidth = actionsVisible ? topbarActionsRef.getBoundingClientRect().width : 0;
		const singleLineGapCount = actionsWidth > 0 ? 3 : 2;
		const singleLineNeeded =
			brandWidth + navWidth + stripWidth + actionsWidth + rowGap * singleLineGapCount;
		const shouldUseMultiline = singleLineNeeded > rowWidth;

		headerMultiline = shouldUseMultiline;
		navAndMetricStacked =
			(shouldUseMultiline || window.innerWidth <= 920) && navWidth + stripWidth + rowGap > rowWidth;
	}

	function scheduleHeaderLayoutRefresh() {
		if (typeof window === 'undefined') return;
		if (headerLayoutRaf) {
			window.cancelAnimationFrame(headerLayoutRaf);
		}

		headerLayoutRaf = window.requestAnimationFrame(() => {
			recalculateHeaderLayout();
			headerLayoutRaf = null;
		});
	}

	$effect(() => {
		if (typeof window === 'undefined' || typeof document === 'undefined') return;

		if (headerLayoutRaf) {
			window.cancelAnimationFrame(headerLayoutRaf);
			headerLayoutRaf = null;
		}

		if (headerLayoutResizeObserver) {
			headerLayoutResizeObserver.disconnect();
			headerLayoutResizeObserver = null;
		}

		recalculateHeaderLayout();
		window.addEventListener('resize', scheduleHeaderLayoutRefresh, { passive: true });

		headerLayoutResizeObserver = new ResizeObserver(() => {
			scheduleHeaderLayoutRefresh();
		});
		if (topbarRowRef) headerLayoutResizeObserver.observe(topbarRowRef);
		if (brandRef) headerLayoutResizeObserver.observe(brandRef);
		if (primaryNavRef) headerLayoutResizeObserver.observe(primaryNavRef);
		if (liveStripRef) headerLayoutResizeObserver.observe(liveStripRef);
		if (topbarActionsRef) headerLayoutResizeObserver.observe(topbarActionsRef);

		return () => {
			window.removeEventListener('resize', scheduleHeaderLayoutRefresh);
			if (headerLayoutRaf) {
				window.cancelAnimationFrame(headerLayoutRaf);
				headerLayoutRaf = null;
			}
			if (headerLayoutResizeObserver) {
				headerLayoutResizeObserver.disconnect();
				headerLayoutResizeObserver = null;
			}
		};
	});

	onMount(() => {
		let cancelled = false;
		let refreshInterval: number | undefined;
		let refreshTimeout: number | undefined;

		function pageVisible() {
			return document.visibilityState === 'visible';
		}

		async function refreshLiveStatus() {
			if (!pageVisible()) return;

			try {
				const response = await fetch(`${data.backendBaseUrl}/api/live-status`);
				if (!response.ok) {
					throw new Error(`Backend returned ${response.status}`);
				}

				const payload = (await response.json()) as LiveStatusResponse;
				if (cancelled) return;
				liveStatusContext.liveStatus = payload;
				liveStatusContext.estimatedNowMs = Date.now();
				liveStatusContext.liveStatusError = null;
			} catch (error) {
				if (cancelled) return;
				liveStatusContext.liveStatusError =
					error instanceof Error ? error.message : 'Unable to refresh live status';
			}
		}

		function tickLiveSlot() {
			if (!pageVisible() || !liveStatusContext.liveStatus) return;
			liveStatusContext.estimatedNowMs = Date.now();
		}

		function startLiveRefresh() {
			void refreshLiveStatus();
			refreshInterval = window.setInterval(refreshLiveStatus, LIVE_REFRESH_MS);
		}

		function handleVisibilityChange() {
			if (!pageVisible()) return;
			liveStatusContext.estimatedNowMs = Date.now();
			void refreshLiveStatus();
		}

		refreshTimeout = window.setTimeout(
			startLiveRefresh,
			liveStatusContext.liveStatus ? INITIAL_REFRESH_DELAY_MS : 0
		);
		const tickInterval = window.setInterval(tickLiveSlot, SLOT_TICK_MS);
		document.addEventListener('visibilitychange', handleVisibilityChange);

		return () => {
			cancelled = true;
			if (refreshInterval) window.clearInterval(refreshInterval);
			if (refreshTimeout) window.clearTimeout(refreshTimeout);
			window.clearInterval(tickInterval);
			document.removeEventListener('visibilitychange', handleVisibilityChange);
		};
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<meta property="og:site_name" content="FireWatch" />
	<meta property="og:image" content={previewImageUrl()} />
	<meta property="og:image:alt" content={previewImageAlt()} />
	<meta property="og:image:type" content="image/png" />
	<meta property="og:image:width" content="1200" />
	<meta property="og:image:height" content="630" />
	<meta name="twitter:card" content="summary_large_image" />
	<meta name="twitter:image" content={previewImageUrl()} />
</svelte:head>

<WalletProvider rpcEndpoint="https://jenifer-6iwpyb-fast-mainnet.helius-rpc.com">
<div class="shell">
	<header class="topbar" class:home-overlay={isHomePage} class:revealed={showTopbar}>
		<div
			class="topbar-row"
			bind:this={topbarRowRef}
			class:multiline-header={headerMultiline}
			class:nav-metrics-stacked={navAndMetricStacked}
		>
			<div class="brand" bind:this={brandRef}>
				<BrandLockup />
			</div>

			<nav class="primary-nav" aria-label="Primary" bind:this={primaryNavRef}>
				{#each primaryNav as item (item.href)}
					<a class:active={isActive(page.url.pathname, item.href)} href={item.href}>
						{item.label}
					</a>
				{/each}
			</nav>

			<div class="topbar-actions" bind:this={topbarActionsRef} class:cta-hidden={!showHeaderCta}>
				<a
					class="header-cta"
					class:visible={showHeaderCta}
					href={addBotUrl()}
					rel="noreferrer"
					target="_blank"
				>
					Add {botHandle()}
				</a>
			</div>

			<div class="live-strip" bind:this={liveStripRef}>
				<LiveStatusStrip status={liveStatusContext} />
			</div>
		</div>

		{#if data.backendError && !liveStatusContext.liveStatus}
			<p class="banner error">Backend unavailable: {data.backendError}</p>
		{:else if liveStatusContext.liveStatusError && !liveStatusContext.liveStatus}
			<p class="banner">Live refresh delayed: {liveStatusContext.liveStatusError}</p>
		{/if}
	</header>

	<main class="content">
		{@render children()}
	</main>

	<SiteFooter />
</div>
</WalletProvider>

<style>
	:global(body) {
		--fw-ink: #17202a;
		--fw-muted: #5f6c79;
		--fw-border: rgba(23, 32, 42, 0.08);
		--fw-accent: #bd4a17;
		--fw-accent-soft: rgba(255, 123, 54, 0.12);
		--fw-page-max: 1160px;
		--fw-page-gutter: 1rem;
		--fw-panel: #ffffff;
		--fw-panel-soft: #fbfaf8;
		margin: 0;
		font-family: 'IBM Plex Sans', 'Avenir Next', sans-serif;
		background: #f8f5f0;
		color: var(--fw-ink);
		min-width: 320px;
		-webkit-font-smoothing: antialiased;
		text-rendering: optimizeLegibility;
	}

	:global(html,
	body,
	#svelte) {
		min-height: 100%;
		height: 100%;
	}

	:global(body) {
		min-height: 100dvh;
		width: 100%;
	}

	:global(*) {
		box-sizing: border-box;
	}

	:global(a) {
		color: inherit;
	}

	:global(code) {
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
	}

	.shell {
		display: flex;
		flex-direction: column;
		min-height: 100dvh;
		min-height: 100vh;
		width: 100%;
	}

	.topbar {
		position: sticky;
		top: 0;
		z-index: 30;
		padding: 0.68rem 1rem 0.6rem;
		background: #f8f5f0;
		border-bottom: 1px solid var(--fw-border);
	}

	.topbar.home-overlay {
		position: fixed;
		left: 0;
		right: 0;
		opacity: 0;
		pointer-events: none;
		visibility: hidden;
		transition:
			opacity 140ms ease,
			visibility 0s linear 140ms;
	}

	.topbar.home-overlay.revealed {
		opacity: 1;
		pointer-events: auto;
		visibility: visible;
		transition: opacity 140ms ease;
	}

	.topbar-row {
		width: min(var(--fw-page-max), calc(100vw - (var(--fw-page-gutter) * 2)));
		margin: 0 auto;
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto auto;
		align-items: center;
		gap: 0.9rem;
	}

	.brand {
		text-decoration: none;
	}

	.primary-nav {
		display: flex;
		flex-wrap: wrap;
		gap: 0.85rem;
		min-width: 0;
	}

	.primary-nav a {
		padding: 0.42rem 0 0.5rem;
		border-bottom: 2px solid transparent;
		text-decoration: none;
		font-weight: 700;
		font-size: 0.86rem;
		color: #556473;
	}

	.primary-nav a.active {
		color: var(--fw-accent);
		border-bottom-color: var(--fw-accent);
	}

	.topbar-actions {
		display: flex;
		align-items: center;
		justify-self: end;
		justify-content: flex-end;
		grid-column: 4;
		grid-row: 1;
		min-width: 0;
	}

	.topbar-actions.cta-hidden {
		display: none;
	}

	.live-strip {
		display: flex;
		flex-wrap: nowrap;
		gap: 0;
		min-width: 0;
		grid-column: 3;
		grid-row: 1;
	}

	.header-cta {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		margin-left: auto;
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
		opacity: 0;
		pointer-events: none;
		transition: opacity 140ms ease;
	}

	.header-cta.visible {
		opacity: 1;
		pointer-events: auto;
	}

	.banner {
		width: min(var(--fw-page-max), calc(100vw - (var(--fw-page-gutter) * 2)));
		margin: 0.62rem auto 0;
		padding: 0.7rem 0.85rem;
		border-radius: 8px;
		background: rgba(255, 196, 112, 0.24);
		color: #7b5717;
		font-size: 0.88rem;
		font-weight: 600;
	}

	.banner.error {
		background: rgba(191, 74, 57, 0.14);
		color: #9c3929;
	}

	.content {
		flex: 1 1 auto;
		min-height: 0;
		padding-bottom: 0;
	}

	:global(.site-footer) {
		margin-top: auto;
		padding-top: 1.2rem;
	}

	.topbar-row.multiline-header {
		grid-template-columns: 1fr auto;
		grid-template-rows: auto auto auto;
		align-items: start;
		gap: 0.58rem;
	}

	:where(.topbar-row.multiline-header) .brand {
		grid-column: 1;
		grid-row: 1;
	}

	:where(.topbar-row.multiline-header) .primary-nav {
		grid-column: 1;
		grid-row: 2;
		flex-wrap: nowrap;
		gap: 0.72rem;
		overflow-x: auto;
		padding-bottom: 0.02rem;
		scrollbar-width: none;
	}

	:where(.topbar-row.multiline-header.nav-metrics-stacked) .primary-nav {
		grid-column: 1 / -1;
		grid-row: 2;
	}

	:where(.topbar-row.multiline-header) .primary-nav::-webkit-scrollbar,
	:where(.topbar-row.multiline-header) .live-strip::-webkit-scrollbar {
		display: none;
	}

	:where(.topbar-row.multiline-header) .topbar-actions {
		grid-column: 2;
		grid-row: 1;
		justify-content: flex-end;
		align-items: flex-end;
		gap: 0.6rem;
		justify-self: end;
	}

	:where(.topbar-row.multiline-header) .live-strip {
		grid-column: 2;
		grid-row: 2;
		justify-content: flex-end;
		overflow-x: auto;
		overflow-y: hidden;
		padding-top: 0.1rem;
		margin-left: 0;
	}

	:where(.topbar-row.multiline-header.nav-metrics-stacked) .live-strip {
		grid-column: 1 / -1;
		grid-row: 3;
		width: 100%;
		justify-content: flex-start;
	}

	:where(.topbar-row.multiline-header) .header-cta {
		order: 0;
	}

	@media (max-width: 920px) {
		.topbar-row {
			grid-template-columns: 1fr auto;
			grid-template-rows: auto auto auto;
			align-items: start;
			gap: 0.58rem;
		}

		.brand {
			grid-column: 1;
			grid-row: 1;
		}

		.primary-nav {
			grid-column: 1;
			grid-row: 2;
			flex-wrap: nowrap;
			gap: 0.72rem;
			overflow-x: auto;
			padding-bottom: 0.02rem;
			scrollbar-width: none;
		}

		.topbar-row.nav-metrics-stacked .primary-nav {
			grid-column: 1 / -1;
			grid-row: 2;
		}

		.topbar-row:not(.nav-metrics-stacked) .primary-nav {
			grid-column: 1;
			grid-row: 2;
		}

		.primary-nav::-webkit-scrollbar,
		.live-strip::-webkit-scrollbar {
			display: none;
		}

		.topbar-actions {
			grid-column: 2;
			grid-row: 1;
			justify-content: flex-end;
			align-items: flex-end;
			gap: 0.6rem;
			justify-self: end;
		}

		.live-strip {
			grid-column: 2;
			grid-row: 2;
			justify-content: flex-end;
			overflow-x: auto;
			overflow-y: hidden;
			padding-top: 0.1rem;
			margin-left: 0;
		}

		.topbar-row.nav-metrics-stacked .live-strip {
			grid-column: 1 / -1;
			grid-row: 3;
			width: 100%;
			justify-content: flex-start;
		}

		.topbar-row:not(.nav-metrics-stacked) .live-strip {
			grid-column: 2;
			grid-row: 2;
			justify-content: flex-end;
			width: auto;
		}

		.header-cta {
			order: 0;
		}
	}

	@media (max-width: 560px) {
		.topbar {
			padding: 0.52rem 0.75rem 0.5rem;
		}

		.topbar-row,
		.banner {
			width: 100%;
		}

		.primary-nav {
			flex-wrap: nowrap;
			gap: 0.72rem;
			overflow-x: auto;
			padding-bottom: 0.02rem;
			scrollbar-width: none;
		}

		.primary-nav a {
			flex: 0 0 auto;
			padding: 0.28rem 0 0.34rem;
			font-size: 0.82rem;
		}

		.live-strip {
			display: flex;
			align-items: center;
			flex-wrap: nowrap;
			gap: 0;
			max-width: 100%;
			overflow-x: auto;
			scrollbar-width: none;
		}

		.topbar-actions {
			grid-column: 2;
			grid-row: 1;
			justify-self: end;
			justify-content: flex-end;
			width: auto;
			gap: 0.42rem;
			flex-direction: row;
		}

		.header-cta {
			order: -1;
			min-height: 1.95rem;
			font-size: 0.76rem;
		}

		.banner {
			margin-top: 0.5rem;
			padding: 0.58rem 0.68rem;
			font-size: 0.82rem;
		}
	}
</style>
