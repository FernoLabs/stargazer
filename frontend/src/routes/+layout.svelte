<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';

	let { children } = $props();
	let isProductApp = $derived(page.url.pathname === '/app' || page.url.pathname.startsWith('/app/'));
	let isDocs = $derived(page.url.pathname === '/docs' || page.url.pathname.startsWith('/docs/'));

	const navigation = [
		{ href: '/#product', label: 'Product' },
		{ href: '/#signals', label: 'How it works' },
		{ href: '/docs/architecture', label: 'Documentation' },
		{ href: '/#roadmap', label: 'Roadmap' }
	] as const;
</script>

<svelte:head>
	<link rel="preconnect" href="https://fonts.googleapis.com" />
	<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
	<link
		href="https://fonts.googleapis.com/css2?family=Archivo+SemiCondensed:wght@500;600&family=Fragment+Mono&family=Source+Sans+3:wght@400;500;600&display=swap"
		rel="stylesheet"
	/>
	<meta property="og:site_name" content="Stargazer by Ferno" />
	<meta name="theme-color" content="#03040b" />
</svelte:head>

{#if isProductApp}
	{@render children()}
{:else}
	<div class={['watch-shell', { 'docs-mode': isDocs }]}>
		{#if !isDocs}
			<header class="watch-masthead">
			<div class="watch-masthead-inner">
				<a class="watch-wordmark" href={resolve('/')} aria-label="Stargazer by Ferno home">
					<svg viewBox="0 0 32 32" aria-hidden="true">
						<path d="M16 1c0 8.7-4 13-13 14 9 1 13 5.3 13 14 0-8.7 4-13 13-14-9-1-13-5.3-13-14Z" />
						<circle cx="25.5" cy="6.5" r="1.6" />
					</svg>
					<span><strong>STARGAZER</strong><i>BY FERNO</i></span>
				</a>

				<nav class="watch-nav" aria-label="Stargazer">
					{#each navigation as item (item.href)}
						<a
							href={resolve(item.href)}
							aria-current={item.href === '/docs/architecture' && isDocs
								? 'page'
								: undefined}
						>
							{item.label}
						</a>
					{/each}
				</nav>

				<a class="watch-contact" href={resolve('/app/alerts')}>Explore demo <span>→</span></a>
			</div>
			</header>
		{/if}

		{#if isDocs}
			{@render children()}
		{:else}
			<main class="watch-content">
				{@render children()}
			</main>
		{/if}

		{#if !isDocs}
			<footer class="watch-footer">
			<div class="watch-footer-inner">
				<div class="footer-brand">
					<strong>STARGAZER</strong>
					<p>Protocol intelligence for the infrastructure Stellar applications depend on.</p>
				</div>
				<nav aria-label="Stargazer footer">
					{#each navigation as item (item.href)}
						<a href={resolve(item.href)}>{item.label}</a>
					{/each}
					<a href={resolve('/app/alerts')}>Stellar demo</a>
					<a href="https://ferno.ag" rel="noreferrer" target="_blank">Ferno.ag ↗</a>
				</nav>
				<p class="watch-legal">
					Independent research project. Stellar is a trademark of the Stellar Development Foundation.
				</p>
			</div>
			</footer>
		{/if}
	</div>
{/if}

<style>
	:global(html),
	:global(body) {
		min-height: 100%;
		scroll-behavior: smooth;
	}

	:global(body) {
		margin: 0;
		min-width: 320px;
		min-height: 100dvh;
		background: #03040b;
		color: #e8ecf6;
		font-family: 'Source Sans 3', sans-serif;
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
		font-family: 'Fragment Mono', monospace;
	}

	@media (prefers-reduced-motion: reduce) {
		:global(html) {
			scroll-behavior: auto;
		}
	}

	.watch-shell {
		--watch-field: #03040b;
		--watch-deep: #000209;
		--watch-surface: #080d1b;
		--watch-ink: #e8ecf6;
		--watch-muted: #99a3b8;
		--watch-rule: #182440;
		--watch-rule-strong: #2a3c68;
		--watch-blue: #617fe8;
		--watch-blue-soft: #a7c2ff;
		display: flex;
		flex-direction: column;
		min-height: 100dvh;
		background:
			radial-gradient(circle at 85% 8%, rgba(70, 98, 202, 0.13), transparent 24rem),
			var(--watch-field);
		color: var(--watch-ink);
	}

	.watch-shell.docs-mode {
		background: #07090f;
	}

	.watch-masthead {
		position: relative;
		z-index: 10;
		border-bottom: 1px solid var(--watch-rule);
		background: rgba(3, 4, 11, 0.94);
		backdrop-filter: blur(14px);
	}

	.watch-masthead-inner,
	.watch-footer-inner {
		width: min(1320px, calc(100vw - 4rem));
		margin: 0 auto;
	}

	.watch-masthead-inner {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: center;
		gap: 2rem;
		min-height: 4.25rem;
	}

	.watch-wordmark {
		display: inline-flex;
		align-items: center;
		gap: 0.7rem;
		font-family: 'Archivo SemiCondensed', sans-serif;
		text-decoration: none;
	}

	.watch-wordmark svg {
		width: 1.8rem;
		height: 1.8rem;
		fill: var(--watch-blue-soft);
		filter: drop-shadow(0 0 0.45rem rgba(97, 127, 232, 0.55));
	}

	.watch-wordmark span {
		display: inline-flex;
		align-items: baseline;
		gap: 0.55rem;
		font-size: 0.9rem;
		letter-spacing: 0.14em;
	}

	.watch-wordmark i {
		color: var(--watch-blue-soft);
		font-size: 0.58rem;
		font-style: normal;
		font-weight: 600;
		letter-spacing: 0.1em;
	}

	.watch-nav {
		display: flex;
		justify-content: center;
		gap: 1.65rem;
	}

	.watch-nav a,
	.watch-contact,
	.watch-footer a {
		font-size: 0.8rem;
		font-weight: 600;
		text-decoration: none;
	}

	.watch-nav a {
		color: var(--watch-muted);
	}

	.watch-nav a:hover,
	.watch-footer a:hover {
		color: var(--watch-blue-soft);
	}

	.watch-nav a[aria-current='page'] {
		color: var(--watch-blue-soft);
	}

	.watch-contact {
		display: inline-flex;
		justify-self: end;
		gap: 1.15rem;
		padding: 0.65rem 0.8rem;
		border: 1px solid var(--watch-rule-strong);
	}

	.watch-contact:hover {
		border-color: var(--watch-blue-soft);
		color: var(--watch-blue-soft);
	}

	.watch-content {
		flex: 1 1 auto;
	}

	.watch-footer {
		padding: 3rem 0;
		border-top: 1px solid var(--watch-rule);
		background: var(--watch-deep);
	}

	.watch-footer-inner {
		display: grid;
		grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr);
		gap: 2rem;
		align-items: start;
	}

	.footer-brand strong {
		font: 600 0.95rem/1.3 'Archivo SemiCondensed', sans-serif;
		letter-spacing: 0.14em;
	}

	.watch-footer p {
		max-width: 34rem;
		margin: 0.5rem 0 0;
		color: var(--watch-muted);
		font-size: 0.8rem;
		line-height: 1.5;
	}

	.watch-footer nav {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: 0.75rem 1.1rem;
	}

	.watch-footer .watch-legal {
		grid-column: 1 / -1;
		margin-top: 1rem;
		padding-top: 1rem;
		border-top: 1px solid var(--watch-rule);
		font-size: 0.7rem;
	}

	@media (max-width: 980px) {
		.watch-masthead-inner {
			grid-template-columns: auto minmax(0, 1fr);
			padding-top: 0.75rem;
		}

		.watch-nav {
			grid-column: 1 / -1;
			grid-row: 2;
			justify-content: flex-start;
			padding: 0.7rem 0;
			border-top: 1px solid var(--watch-rule);
		}

		.watch-contact {
			grid-column: 2;
			grid-row: 1;
		}
	}

	@media (max-width: 560px) {
		.watch-masthead-inner,
		.watch-footer-inner {
			width: min(100vw - 2rem, 1320px);
		}

		.watch-masthead-inner {
			gap: 0.75rem;
		}

		.watch-wordmark span {
			font-size: 0.78rem;
		}

		.watch-wordmark i {
			display: none;
		}

		.watch-nav {
			gap: 1.1rem;
			overflow-x: auto;
			white-space: nowrap;
			scrollbar-width: none;
		}

		.watch-nav::-webkit-scrollbar {
			display: none;
		}

		.watch-contact {
			font-size: 0.72rem;
		}

		.watch-footer-inner {
			grid-template-columns: 1fr;
		}

		.watch-footer nav {
			justify-content: flex-start;
		}

		.watch-footer .watch-legal {
			grid-column: 1;
		}
	}
</style>
