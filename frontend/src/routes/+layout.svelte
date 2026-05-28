<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import favicon from '$lib/assets/firewatch-tower-icon-favicon.png';
	import BrandLockup from '$lib/components/BrandLockup.svelte';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import type { LiveStatusResponse } from '$lib/types';

	let { data, children } = $props();
	type PreviewMeta = {
		imageUrl?: string;
		alertLabel?: string;
	};

	const SLOT_TICK_MS = 1000;
	const LIVE_REFRESH_MS = 10_000;
	const INITIAL_REFRESH_DELAY_MS = 6500;
	const countFormatter = new Intl.NumberFormat('en-US');

	const primaryNav = [
		{ href: '/', label: 'Watch' },
		{ href: '/programs', label: 'Programs' },
		{ href: '/events', label: 'Activity' },
		{ href: '/pricing', label: 'Pricing' },
		{ href: '/docs', label: 'Reference' }
	];

	let liveStatus = $state<LiveStatusResponse | null>(null);
	let liveStatusError = $state<string | null>(null);
	let estimatedNowMs = $state(Date.now());

	$effect(() => {
		liveStatus = data.liveStatus ?? null;
	});

	function isActive(pathname: string, href: string) {
		if (href === '/') return pathname === '/';
		return pathname === href || pathname.startsWith(`${href}/`);
	}

	function currentLiveStatus() {
		return liveStatus ?? data.liveStatus ?? null;
	}

	function currentSummary() {
		return currentLiveStatus()?.summary ?? null;
	}

	function estimatedLiveSlot() {
		const status = currentLiveStatus();
		const baseSlot = status?.summary.last_processed_slot;
		if (baseSlot == null) return null;

		const sampledAtMs = status?.sampled_at_ms;
		const slotTimeMs = Math.max(1, status?.estimated_slot_time_ms ?? SLOT_TICK_MS);
		if (!sampledAtMs) return baseSlot;

		const elapsedMs = Math.max(0, estimatedNowMs - sampledAtMs);
		return baseSlot + Math.floor(elapsedMs / slotTimeMs);
	}

	function indexingStartSlot() {
		const summary = currentSummary();
		return summary?.backfill_start_slot ?? summary?.first_indexed_slot ?? null;
	}

	function formatCount(value: number | null | undefined) {
		return countFormatter.format(value ?? 0);
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
				liveStatus = payload;
				estimatedNowMs = Date.now();
				liveStatusError = null;
			} catch (error) {
				if (cancelled) return;
				liveStatusError = error instanceof Error ? error.message : 'Unable to refresh live status';
			}
		}

		function tickLiveSlot() {
			if (!pageVisible() || !currentLiveStatus()) return;
			estimatedNowMs = Date.now();
		}

		function startLiveRefresh() {
			void refreshLiveStatus();
			refreshInterval = window.setInterval(refreshLiveStatus, LIVE_REFRESH_MS);
		}

		function handleVisibilityChange() {
			if (!pageVisible()) return;
			estimatedNowMs = Date.now();
			void refreshLiveStatus();
		}

		refreshTimeout = window.setTimeout(
			startLiveRefresh,
			currentLiveStatus() ? INITIAL_REFRESH_DELAY_MS : 0
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

<div class="shell">
	<header class="topbar">
		<div class="topbar-row">
			<div class="brand">
				<BrandLockup />
			</div>

			<nav class="primary-nav" aria-label="Primary">
				{#each primaryNav as item}
					<a class:active={isActive(page.url.pathname, item.href)} href={item.href}>
						{item.label}
					</a>
				{/each}
			</nav>

			<div class="live-strip" aria-label="Live status">
				<span class="live-pill tip-pill">
					<span class="tip-main">
						<span>Tip</span>
						<strong>{formatCount(estimatedLiveSlot())}</strong>
					</span>
					{#if indexingStartSlot() != null}
						<span class="tip-start">start {formatCount(indexingStartSlot())}</span>
					{/if}
				</span>
				<span class="live-pill">
					<span>Events</span>
					<strong>{formatCount(currentSummary()?.total_count)}</strong>
				</span>
				<span class="live-pill">
					<span>Watches</span>
					<strong>{formatCount(data.telegramSubscriptionCount)}</strong>
				</span>
			</div>
		</div>

		{#if data.backendError}
			<p class="banner error">Backend unavailable: {data.backendError}</p>
		{:else if liveStatusError && !currentLiveStatus()}
			<p class="banner">Live refresh delayed: {liveStatusError}</p>
		{/if}
	</header>

	<main class="content">
		{@render children()}
	</main>

	<SiteFooter />
</div>

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
		min-height: 100vh;
	}

	.topbar {
		position: sticky;
		top: 0;
		z-index: 30;
		padding: 0.68rem 1rem 0.6rem;
		background: #f8f5f0;
		border-bottom: 1px solid var(--fw-border);
	}

	.topbar-row {
		width: min(var(--fw-page-max), calc(100vw - (var(--fw-page-gutter) * 2)));
		margin: 0 auto;
		display: grid;
		grid-template-columns: auto minmax(0, auto) minmax(0, 1fr);
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

	.live-strip {
		display: flex;
		justify-self: end;
		flex-wrap: wrap;
		gap: 0;
		min-width: 0;
	}

	.live-pill {
		display: inline-flex;
		align-items: baseline;
		gap: 0.34rem;
		min-width: 0;
		padding: 0.1rem 0 0.1rem 0.8rem;
		margin-left: 0.8rem;
		border-left: 1px solid rgba(23, 32, 42, 0.1);
	}

	.live-pill:first-child {
		margin-left: 0;
		padding-left: 0;
		border-left: none;
	}

	.tip-pill {
		display: grid;
		align-items: center;
		gap: 0.02rem;
	}

	.tip-main {
		display: inline-flex;
		align-items: baseline;
		gap: 0.34rem;
	}

	.live-pill span {
		font-size: 0.76rem;
		font-weight: 600;
		color: #677584;
	}

	.live-pill strong {
		font-size: 0.82rem;
		white-space: nowrap;
	}

	.tip-start {
		justify-self: end;
		font-size: 0.64rem;
		line-height: 1;
		color: #8a7666;
		white-space: nowrap;
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
		padding-bottom: 0;
	}

	@media (max-width: 920px) {
		.topbar-row {
			grid-template-columns: 1fr;
			align-items: start;
			gap: 0.58rem;
		}

		.live-strip {
			justify-self: start;
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

		.primary-nav::-webkit-scrollbar,
		.live-strip::-webkit-scrollbar {
			display: none;
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
			width: 100%;
			overflow-x: auto;
			scrollbar-width: none;
		}

		.live-pill {
			display: inline-flex;
			align-items: baseline;
			flex: 0 0 auto;
			gap: 0.28rem;
			margin-left: 0;
			padding: 0 0.62rem 0 0;
			border-left: none;
			border-right: 1px solid rgba(23, 32, 42, 0.1);
		}

		.live-pill:first-child {
			padding-left: 0;
		}

		.live-pill:last-child {
			padding-right: 0;
			border-right: none;
		}

		.live-pill span {
			font-size: 0.72rem;
		}

		.live-pill strong {
			font-size: 0.78rem;
		}

		.tip-pill,
		.tip-main {
			display: inline-flex;
			align-items: baseline;
		}

		.tip-start {
			display: none;
		}

		.banner {
			margin-top: 0.5rem;
			padding: 0.58rem 0.68rem;
			font-size: 0.82rem;
		}
	}
</style>
