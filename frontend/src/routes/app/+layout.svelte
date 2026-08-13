<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();

	const navigation = [
		{
			href: '/app',
			label: 'Wallet',
			matches: (path: string) =>
				path === '/app' || path.startsWith('/app/wallet/') || path.startsWith('/app/watch/')
		},
		{
			href: '/app/leaderboard',
			label: 'Wallet activity',
			matches: (path: string) => path === '/app/leaderboard'
		},
		{
			href: '/app/alerts',
			label: 'Alerts',
			matches: (path: string) => path === '/app/alerts'
		},
		{
			href: '/app/protocols',
			label: 'Protocols',
			matches: (path: string) => path === '/app/protocols'
		}
	] as const;
</script>

<svelte:head>
	<meta name="theme-color" content="#050814" />
</svelte:head>

<div class="app-shell">
	<header class="app-header">
		<div class="app-header-inner">
			<a class="app-wordmark" href={resolve('/app')} aria-label="Stargazer Stellar demo home">
				<svg viewBox="0 0 32 32" aria-hidden="true">
					<path d="M16 1c0 8.7-4 13-13 14 9 1 13 5.3 13 14 0-8.7 4-13 13-14-9-1-13-5.3-13-14Z" />
					<circle cx="25.5" cy="6.5" r="1.6" />
				</svg>
				<span><strong>STARGAZER</strong><i>STELLAR DEMO</i></span>
			</a>

			<nav class="app-nav" aria-label="Product demo">
				{#each navigation as item (item.href)}
					<a
						href={resolve(item.href)}
						aria-current={item.matches(page.url.pathname) ? 'page' : undefined}
					>
						{item.label}
					</a>
				{/each}
			</nav>

			<div class="app-header-meta">
				<span>Indexed Stellar Mainnet</span>
				<a href={resolve('/')}>Back to Stargazer</a>
			</div>
		</div>
	</header>

	<main class="app-main">
		{@render children()}
	</main>
</div>

<style>
	:global(body) {
		background: #f3f5f9;
	}

	.app-shell {
		--app-field: #f3f5f9;
		--app-paper: #ffffff;
		--app-ink: #111827;
		--app-muted: #667085;
		--app-rule: #d9dee8;
		--app-rule-strong: #aab4c7;
		--app-violet: #4968d8;
		--app-violet-soft: #e8edff;
		--app-danger: #c43d58;
		--app-warning: #9a650f;
		--app-success: #28745a;
		min-height: 100dvh;
		background: var(--app-field);
		color: var(--app-ink);
		font-family: 'Source Sans 3', sans-serif;
	}

	.app-header {
		border-bottom: 1px solid #1c2946;
		background: #050814;
		color: #e8ecf6;
	}

	.app-header-inner,
	.app-main {
		width: min(1200px, calc(100vw - 3rem));
		margin: 0 auto;
	}

	.app-header-inner {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: center;
		gap: 2.25rem;
		min-height: 3.75rem;
	}

	.app-wordmark {
		display: inline-flex;
		align-items: center;
		gap: 0.7rem;
		font-family: 'Archivo SemiCondensed', sans-serif;
		text-decoration: none;
	}

	.app-wordmark svg {
		width: 1.65rem;
		height: 1.65rem;
		fill: #a7c2ff;
	}

	.app-wordmark span {
		display: flex;
		align-items: baseline;
		gap: 0.55rem;
		font-size: 0.88rem;
		letter-spacing: 0.12em;
	}

	.app-wordmark i {
		color: #a7c2ff;
		font-size: 0.58rem;
		font-style: normal;
		font-weight: 600;
		letter-spacing: 0.08em;
	}

	.app-nav {
		display: flex;
		align-self: stretch;
		gap: 1.35rem;
	}

	.app-nav a {
		display: inline-flex;
		align-items: center;
		border-bottom: 2px solid transparent;
		color: #99a3b8;
		font-size: 0.875rem;
		font-weight: 600;
		text-decoration: none;
	}

	.app-nav a:hover {
		color: #e8ecf6;
	}

	.app-nav a[aria-current='page'] {
		border-bottom-color: #7896ff;
		color: #ffffff;
	}

	.app-header-meta {
		display: flex;
		align-items: center;
		gap: 1rem;
		font-size: 0.78rem;
	}

	.app-header-meta span {
		color: #a7c2ff;
		font-family: 'Fragment Mono', monospace;
	}

	.app-header-meta a {
		color: #c8d0e0;
		font-weight: 600;
		text-decoration: none;
	}

	.app-header-meta a:hover {
		color: #a7c2ff;
	}

	.app-main {
		padding: 2rem 0 4.5rem;
	}

	@media (max-width: 1040px) {
		.app-header-inner {
			grid-template-columns: auto 1fr;
			padding-top: 0.7rem;
		}

		.app-nav {
			grid-column: 1 / -1;
			grid-row: 2;
			min-height: 2.8rem;
			border-top: 1px solid #1c2946;
		}

		.app-header-meta {
			justify-self: end;
		}
	}

	@media (max-width: 680px) {
		.app-header-inner,
		.app-main {
			width: min(100vw - 2rem, 1200px);
		}

		.app-header-inner {
			gap: 0.75rem;
		}

		.app-wordmark i,
		.app-header-meta span {
			display: none;
		}

		.app-nav {
			gap: 1.25rem;
			overflow-x: auto;
			white-space: nowrap;
		}

		.app-main {
			padding-top: 1.5rem;
		}
	}
</style>
