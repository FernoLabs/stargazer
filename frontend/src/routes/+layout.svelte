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
	<meta property="og:site_name" content="Ferno Pulsar" />
	<meta name="theme-color" content="#f4f5f2" />
</svelte:head>

<div class="pulsar-shell">
	<header class="pulsar-masthead">
		<div class="pulsar-masthead-inner">
			<a class="pulsar-wordmark" href={resolve('/')} aria-label="Ferno Pulsar home">
				<svg viewBox="0 0 36 24" aria-hidden="true">
					<path d="M1 12h7l2.8-8 4.4 16 4.5-16 3.7 8H35" />
				</svg>
				<span><strong>FERNO</strong><i>/</i><strong>PULSAR</strong></span>
			</a>

			<nav class="pulsar-nav" aria-label="Ferno Pulsar">
				{#each navigation as item (item.href)}
					<a href={resolve(item.href)}>{item.label}</a>
				{/each}
			</nav>

			<a class="pulsar-contact" href="https://x.com/ferno_ag" rel="noreferrer" target="_blank">
				Pilot enquiries ↗
			</a>
		</div>
	</header>

	<main class="pulsar-content">
		{@render children()}
	</main>

	<footer class="pulsar-footer">
		<div class="pulsar-footer-inner">
			<div>
				<strong>Ferno / Pulsar</strong>
				<p>Post-deployment monitoring for Stellar protocol changes and dependencies.</p>
			</div>
			<nav aria-label="Ferno Pulsar footer">
				{#each navigation as item (item.href)}
					<a href={resolve(item.href)}>{item.label}</a>
				{/each}
				<a href="https://ferno.ag" rel="noreferrer" target="_blank">Ferno.ag ↗</a>
			</nav>
			<p class="pulsar-legal">
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

	.pulsar-shell {
		--pulsar-field: #f4f5f2;
		--pulsar-ink: #151418;
		--pulsar-rule: #d6d7d0;
		--pulsar-violet: #6436e8;
		display: flex;
		flex-direction: column;
		min-height: 100dvh;
		background: var(--pulsar-field);
		color: var(--pulsar-ink);
	}

	.pulsar-masthead {
		border-bottom: 1px solid var(--pulsar-rule);
		background: var(--pulsar-field);
	}

	.pulsar-masthead-inner,
	.pulsar-footer-inner {
		width: min(1320px, calc(100vw - 4rem));
		margin: 0 auto;
	}

	.pulsar-masthead-inner {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: center;
		gap: 2rem;
		min-height: 4.25rem;
	}

	.pulsar-wordmark {
		display: inline-flex;
		align-items: center;
		gap: 0.7rem;
		font-family: 'Archivo SemiCondensed', sans-serif;
		text-decoration: none;
	}

	.pulsar-wordmark svg {
		width: 2.15rem;
		height: 1.45rem;
		fill: none;
		stroke: var(--pulsar-violet);
		stroke-linecap: square;
		stroke-linejoin: miter;
		stroke-width: 2;
	}

	.pulsar-wordmark span {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
		font-size: 0.9rem;
		letter-spacing: 0.035em;
	}

	.pulsar-wordmark i {
		color: var(--pulsar-violet);
		font-style: normal;
		font-weight: 400;
	}

	.pulsar-nav {
		display: flex;
		justify-content: center;
		gap: 1.5rem;
	}

	.pulsar-nav a,
	.pulsar-contact,
	.pulsar-footer a {
		font-size: 0.8rem;
		font-weight: 600;
		text-decoration: none;
	}

	.pulsar-nav a:hover,
	.pulsar-contact:hover,
	.pulsar-footer a:hover {
		color: var(--pulsar-violet);
		text-decoration: underline;
		text-underline-offset: 0.2em;
	}

	.pulsar-contact {
		justify-self: end;
	}

	.pulsar-content {
		flex: 1 1 auto;
	}

	.pulsar-footer {
		padding: 2.5rem 0;
		border-top: 1px solid var(--pulsar-rule);
		background: #fff;
	}

	.pulsar-footer-inner {
		display: grid;
		grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr);
		gap: 2rem;
		align-items: start;
	}

	.pulsar-footer strong {
		font-family: 'Archivo SemiCondensed', sans-serif;
		font-size: 1rem;
	}

	.pulsar-footer p {
		margin: 0.35rem 0 0;
		max-width: 34rem;
		color: #67636c;
		font-size: 0.8rem;
		line-height: 1.5;
	}

	.pulsar-footer nav {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: 0.75rem 1.1rem;
	}

	.pulsar-footer .pulsar-legal {
		grid-column: 1 / -1;
		margin-top: 1rem;
		padding-top: 1rem;
		border-top: 1px solid var(--pulsar-rule);
		font-size: 0.7rem;
	}

	@media (max-width: 980px) {
		.pulsar-masthead-inner {
			grid-template-columns: auto minmax(0, 1fr);
			padding-top: 0.75rem;
		}

		.pulsar-nav {
			grid-column: 1 / -1;
			grid-row: 2;
			justify-content: flex-start;
			padding: 0.7rem 0;
			border-top: 1px solid var(--pulsar-rule);
		}

		.pulsar-contact {
			grid-column: 2;
			grid-row: 1;
		}
	}

	@media (max-width: 560px) {
		.pulsar-masthead-inner,
		.pulsar-footer-inner {
			width: min(100vw - 2rem, 1320px);
		}

		.pulsar-masthead-inner {
			gap: 0.75rem;
		}

		.pulsar-wordmark span {
			font-size: 0.78rem;
		}

		.pulsar-nav {
			gap: 1rem;
			overflow-x: auto;
			white-space: nowrap;
			scrollbar-width: none;
		}

		.pulsar-nav::-webkit-scrollbar {
			display: none;
		}

		.pulsar-contact {
			font-size: 0.72rem;
		}

		.pulsar-footer-inner {
			grid-template-columns: 1fr;
		}

		.pulsar-footer nav {
			justify-content: flex-start;
		}

		.pulsar-footer .pulsar-legal {
			grid-column: 1;
		}
	}
</style>
