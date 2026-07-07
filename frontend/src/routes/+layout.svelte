<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import favicon from '$lib/assets/firewatch-tower-icon-favicon.png';
	import { WalletProvider } from '@fernolab/wallet-adapter-svelte';
	import BrandLockup from '$lib/components/BrandLockup.svelte';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import { createLiveStatusContext } from '$lib/live-status-context.svelte';
	import type { LiveStatusResponse } from '$lib/types';

	let { data, children } = $props();

	type PreviewMeta = {
		imageUrl?: string;
		alertLabel?: string;
	};

	type ConfigHeaderData = {
		session?: {
			chat_title?: string | null;
			chat_type?: string | null;
		} | null;
		user?: {
			telegram_username?: string | null;
			telegram_user_id?: number | string | null;
			label?: string | null;
		} | null;
	};

	const SLOT_TICK_MS = 1000;
	const LIVE_REFRESH_MS = 4_000;
	const INITIAL_REFRESH_DELAY_MS = 1500;
	const DEFAULT_BOT_USERNAME = 'fernowatch_bot';

	const primaryNav = [
		{ href: '/', label: 'Overview' },
		{ href: '/protocols', label: 'Protocols' },
		{ href: '/events', label: 'Feed' },
		{ href: '/pricing', label: 'Pricing' }
	] as const;

	const liveStatusContext = createLiveStatusContext(
		() => data.liveStatus ?? null,
		() => data.telegramSubscriptionCount
	);

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

	function addBotUrl() {
		return `https://t.me/${botUsername()}`;
	}

	function openAddBot() {
		window.open(addBotUrl(), '_blank', 'noopener,noreferrer');
	}

	function configHeaderData() {
		return page.data as ConfigHeaderData;
	}

	function hasConfigHeaderSession() {
		return page.url.pathname.startsWith('/config/') && Boolean(configHeaderData().session && configHeaderData().user);
	}

	function configHeaderUserLabel() {
		const user = configHeaderData().user;
		if (!user) return 'Connected';
		if (user.telegram_username) return `@${user.telegram_username}`;
		if (user.telegram_user_id) return String(user.telegram_user_id);
		return user.label ?? 'Connected';
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
				const response = await fetch(resolve('/api/live-status'));
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

<WalletProvider rpcEndpoint="https://api.mainnet-beta.solana.com">
	<div class="shell">
		<header class="topbar">
			<div class="topbar-row">
				<div class="brand">
					<BrandLockup compact />
				</div>

				<nav class="primary-nav" aria-label="Primary">
					{#each primaryNav as item (item.href)}
						<a class={isActive(page.url.pathname, item.href) ? 'active' : ''} href={resolve(item.href)}>
							{item.label}
						</a>
					{/each}
				</nav>

				{#if hasConfigHeaderSession()}
					<form class="header-session" method="POST" action="?/forget_config">
						<a href={resolve('/config')}>{configHeaderUserLabel()}</a>
						<button type="submit">Disconnect</button>
					</form>
				{:else}
					<button class="header-cta" type="button" onclick={openAddBot}>
						<span class="cta-long">Open Telegram bot</span>
						<span class="cta-short">Open bot</span>
					</button>
				{/if}
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
		--fw-border: rgba(23, 32, 42, 0.1);
		--fw-accent: #ad4e22;
		--fw-accent-strong: #8f3d19;
		--fw-success: #2f6f52;
		--fw-page-max: 1180px;
		--fw-page-gutter: 1rem;
		--fw-panel: #ffffff;
		--fw-panel-soft: #f8f7f3;
		margin: 0;
		font-family: 'IBM Plex Sans', 'Avenir Next', sans-serif;
		background: #f5f4ef;
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
		padding: 0.42rem 1rem;
		background: rgba(245, 244, 239, 0.96);
		border-bottom: 1px solid var(--fw-border);
		backdrop-filter: blur(8px);
	}

	.topbar-row {
		width: min(var(--fw-page-max), calc(100vw - (var(--fw-page-gutter) * 2)));
		margin: 0 auto;
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		grid-template-areas: 'brand nav action';
		align-items: center;
		gap: 0.74rem;
	}

	.brand {
		grid-area: brand;
		min-width: 0;
	}

	.brand :global(.brand-lockup) {
		min-height: 2.25rem;
	}

	.primary-nav {
		grid-area: nav;
		display: flex;
		flex-wrap: nowrap;
		gap: 0.68rem;
		min-width: 0;
		overflow-x: auto;
		scrollbar-width: none;
		white-space: nowrap;
	}

	.primary-nav::-webkit-scrollbar {
		display: none;
	}

	.primary-nav a {
		flex: 0 0 auto;
		padding: 0.24rem 0;
		border-bottom: 2px solid transparent;
		color: #53606b;
		font-size: 0.82rem;
		font-weight: 720;
		text-decoration: none;
	}

	.primary-nav a:hover,
	.primary-nav a.active {
		color: #1d252d;
		border-bottom-color: var(--fw-accent);
	}

	.header-cta {
		grid-area: action;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 1.86rem;
		padding: 0 0.62rem;
		border: 1px solid var(--fw-accent);
		border-radius: 7px;
		background: var(--fw-accent);
		color: #fff;
		font: inherit;
		font-size: 0.78rem;
		font-weight: 780;
		text-decoration: none;
		white-space: nowrap;
		cursor: pointer;
	}

	.header-cta:hover {
		background: var(--fw-accent-strong);
		border-color: var(--fw-accent-strong);
	}

	.header-session {
		grid-area: action;
		display: inline-flex;
		align-items: center;
		justify-content: flex-end;
		gap: 0.44rem;
		min-width: 0;
	}

	.header-session a {
		max-width: 10rem;
		overflow: hidden;
		color: #53606b;
		font-size: 0.78rem;
		font-weight: 780;
		text-decoration: none;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.header-session a:hover {
		color: var(--fw-accent-strong);
		text-decoration: underline;
	}

	.header-session button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 1.86rem;
		padding: 0 0.56rem;
		border: 1px solid rgba(23, 32, 42, 0.14);
		border-radius: 7px;
		background: #fffdfa;
		color: #53606b;
		font: inherit;
		font-size: 0.74rem;
		font-weight: 760;
		cursor: pointer;
	}

	.header-session button:hover {
		border-color: rgba(173, 78, 34, 0.4);
		color: var(--fw-accent-strong);
	}

	.cta-short {
		display: none;
	}

	.banner {
		width: min(var(--fw-page-max), calc(100vw - (var(--fw-page-gutter) * 2)));
		margin: 0.62rem auto 0;
		padding: 0.68rem 0.78rem;
		border-radius: 7px;
		background: rgba(138, 94, 30, 0.12);
		color: #6a4b22;
		font-size: 0.86rem;
		font-weight: 650;
	}

	.banner.error {
		background: rgba(156, 57, 41, 0.12);
		color: #9c3929;
	}

	.content {
		flex: 1 1 auto;
		min-height: 0;
	}

	:global(.site-footer) {
		margin-top: auto;
	}

	@media (max-width: 980px) {
		.topbar-row {
			grid-template-columns: auto minmax(0, 1fr) auto;
			gap: 0.58rem;
		}
	}

	@media (max-width: 560px) {
		.topbar {
			padding: 0.5rem 0.5rem 0.42rem;
		}

		.topbar-row,
		.banner {
			width: min(100vw - 1rem, var(--fw-page-max));
		}

		.topbar-row {
			grid-template-columns: minmax(0, 1fr) auto;
			grid-template-areas:
				'brand action'
				'nav nav';
			row-gap: 0.42rem;
			column-gap: 0.6rem;
		}

		.primary-nav {
			margin: 0 -0.06rem;
			padding-top: 0.38rem;
			border-top: 1px solid rgba(23, 32, 42, 0.07);
			gap: 0.85rem;
		}

		.primary-nav a {
			display: inline-flex;
			align-items: center;
			min-height: 2.25rem;
			font-size: 0.78rem;
		}

		.header-cta {
			min-height: 2.25rem;
			padding: 0 0.62rem;
			font-size: 0.74rem;
		}

		.header-session a {
			max-width: 6.5rem;
			font-size: 0.72rem;
		}

		.header-session button {
			min-height: 2.25rem;
			padding: 0 0.56rem;
			font-size: 0.7rem;
		}
	}

	@media (max-width: 430px) {
		.cta-long {
			display: none;
		}

		.cta-short {
			display: inline;
		}
	}
</style>
