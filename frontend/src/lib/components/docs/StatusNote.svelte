<script lang="ts">
	import type { DocsStatus } from '$lib/docs/navigation';
	import type { Snippet } from 'svelte';

	let {
		status,
		title,
		children
	}: { status: DocsStatus; title?: string; children: Snippet } = $props();
</script>

<aside class="status-note" data-status={status} aria-label={`Status: ${status}`}>
	<header>
		<span>{status}</span>
		{#if title}<strong>{title}</strong>{/if}
	</header>
	<div class="status-copy">
		{@render children()}
	</div>
</aside>

<style>
	.status-note {
		--status-color: var(--watch-blue-soft);
		padding: 1rem 1.1rem;
		border: 1px solid var(--watch-rule);
		border-left: 3px solid var(--status-color);
		background: var(--watch-surface);
	}

	.status-note[data-status='Implemented'] {
		--status-color: #73b997;
	}

	.status-note[data-status='In progress'] {
		--status-color: #d3ad62;
	}

	.status-note[data-status='Planned'] {
		--status-color: #8ea8ee;
	}

	header {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 0.5rem 0.8rem;
	}

	header span {
		color: var(--status-color);
		font: 500 0.72rem/1.4 'Fragment Mono', monospace;
	}

	header strong {
		font: 600 0.98rem/1.35 'Source Sans 3', sans-serif;
	}

	.status-copy {
		margin-top: 0.45rem;
		color: var(--watch-muted);
		font-size: 0.92rem;
		line-height: 1.6;
	}

	.status-copy :global(p) {
		margin: 0;
	}

	.status-copy :global(p + p) {
		margin-top: 0.65rem;
	}
</style>
