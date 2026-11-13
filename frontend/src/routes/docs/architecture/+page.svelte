<script lang="ts">
	import { onMount } from 'svelte';
	import architectureHtml from '$repo-docs/ARCHITECTURE.md?html';

	let content: HTMLElement | undefined;

	onMount(() => {
		// The SSR build drops this branch, so the diagram library never enters the server bundle.
		if (!import.meta.env.SSR && content) {
			const root = content;
			void import('$lib/docs/mermaid').then(({ renderMermaidDiagrams }) => renderMermaidDiagrams(root));
		}
	});
</script>

<svelte:head>
	<title>Technical architecture · Stargazer documentation</title>
	<meta
		name="description"
		content="Stargazer technical architecture: live Stellar ingestion, the public control history of Soroban contracts, SEP-48 decoding, the detector catalogue, Telegram and webhook delivery, and the GuardVault protection vault with its Sentinel guardian."
	/>
	<link rel="canonical" href="https://stargazer.ferno.ag/docs/architecture" />
</svelte:head>

<article class="doc-page doc-markdown" bind:this={content}>
	{@html architectureHtml}
	<p class="doc-source-note">
		This page renders
		<a href="https://github.com/FernoLabs/stargazer/blob/main/docs/ARCHITECTURE.md">docs/ARCHITECTURE.md</a>
		from the Stargazer repository.
	</p>
</article>
