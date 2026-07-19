<script lang="ts">
	import { resolve } from '$app/paths';

	let { children } = $props();

	const navigation = [
		{ href: '/#problem', label: 'Problem' },
		{ href: '/#solution', label: 'Solution' },
		{ href: '/#evidence', label: 'Evidence' },
		{ href: '/#roadmap', label: 'Roadmap' }
	] as const;
</script>

<svelte:head>
	<link rel="icon" href="/brand/ferno-logo-white.png" />
	<meta property="og:site_name" content="Stellar Watch by Ferno" />
	<meta name="theme-color" content="#f4f5f2" />
</svelte:head>

<div class="watch-shell">
	<header class="watch-masthead">
		<div class="watch-masthead-inner">
			<a class="watch-wordmark" href={resolve('/')} aria-label="Stellar Watch by Ferno home">
				<svg viewBox="0 0 36 24" aria-hidden="true">
					<path d="M1 12h7l2.8-8 4.4 16 4.5-16 3.7 8H35" />
				</svg>
				<span><strong>STELLAR WATCH</strong><i>BY FERNO</i></span>
			</a>

			<nav class="watch-nav" aria-label="Stellar Watch">
				{#each navigation as item (item.href)}
					<a href={resolve(item.href)}>{item.label}</a>
				{/each}
			</nav>

			<a class="watch-contact" href="https://x.com/ferno_ag" rel="noreferrer" target="_blank">
				Pilot enquiries ↗
			</a>
		</div>
	</header>

	<main class="watch-content">
		{@render children()}
	</main>

	<footer class="watch-footer">
		<div class="watch-footer-inner">
			<div>
				<strong>Stellar Watch by Ferno</strong>
				<p>Post-deployment monitoring for Stellar protocol changes and dependencies.</p>
			</div>
			<nav aria-label="Stellar Watch footer">
				{#each navigation as item (item.href)}
					<a href={resolve(item.href)}>{item.label}</a>
				{/each}
				<a href="https://ferno.ag" rel="noreferrer" target="_blank">Ferno.ag ↗</a>
			</nav>
			<p class="watch-legal">
				Independent research project. Stellar is a trademark of the Stellar Development
				Foundation.
			</p>
		</div>
	</footer>
</div>

<style>
	:global(html),
	:global(body) {
		min-height: 100%;
	}

	:global(body) {
		margin: 0;
		min-width: 320px;
		min-height: 100dvh;
		background: #f4f5f2;
		color: #151418;
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

	.watch-shell {
		--watch-field: #f4f5f2;
		--watch-ink: #151418;
		--watch-rule: #d6d7d0;
		--watch-violet: #6436e8;
		display: flex;
		flex-direction: column;
		min-height: 100dvh;
		background: var(--watch-field);
		color: var(--watch-ink);
	}

	.watch-masthead {
		border-bottom: 1px solid var(--watch-rule);
		background: var(--watch-field);
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
		width: 2.15rem;
		height: 1.45rem;
		fill: none;
		stroke: var(--watch-violet);
		stroke-linecap: square;
		stroke-linejoin: miter;
		stroke-width: 2;
	}

	.watch-wordmark span {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
		font-size: 0.9rem;
		letter-spacing: 0.035em;
	}

	.watch-wordmark i {
		color: var(--watch-violet);
		font-size: 0.62rem;
		font-style: normal;
		font-weight: 600;
		letter-spacing: 0.08em;
	}

	.watch-nav {
		display: flex;
		justify-content: center;
		gap: 1.5rem;
	}

	.watch-nav a,
	.watch-contact,
	.watch-footer a {
		font-size: 0.8rem;
		font-weight: 600;
		text-decoration: none;
	}

	.watch-nav a:hover,
	.watch-contact:hover,
	.watch-footer a:hover {
		color: var(--watch-violet);
		text-decoration: underline;
		text-underline-offset: 0.2em;
	}

	.watch-contact {
		justify-self: end;
	}

	.watch-content {
		flex: 1 1 auto;
	}

	.watch-footer {
		padding: 2.5rem 0;
		border-top: 1px solid var(--watch-rule);
		background: #fff;
	}

	.watch-footer-inner {
		display: grid;
		grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr);
		gap: 2rem;
		align-items: start;
	}

	.watch-footer strong {
		font-family: 'Archivo SemiCondensed', sans-serif;
		font-size: 1rem;
	}

	.watch-footer p {
		margin: 0.35rem 0 0;
		max-width: 34rem;
		color: #67636c;
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

		.watch-nav {
			gap: 1rem;
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
