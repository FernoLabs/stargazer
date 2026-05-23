<script lang="ts">
	let {
		title,
		description = null,
		href = null,
		hrefLabel = null,
		compact = false,
		level = '2'
	}: {
		title: string;
		description?: string | null;
		href?: string | null;
		hrefLabel?: string | null;
		compact?: boolean;
		level?: '1' | '2' | '3';
	} = $props();
</script>

	<div class:compact class="section-header">
		<div class="copy">
			<svelte:element this={`h${level}`}>{title}</svelte:element>
			{#if description}
				<p class="description">{description}</p>
		{/if}
	</div>

	{#if href && hrefLabel}
		<a class="action-link" href={href}>{hrefLabel}</a>
	{/if}
</div>

<style>
	.section-header {
		display: flex;
		flex-wrap: wrap;
		align-items: end;
		justify-content: space-between;
		gap: 0.9rem;
	}

	.copy {
		display: grid;
		gap: 0.18rem;
		min-width: 0;
	}

	h1,
	p {
		margin: 0;
	}

	h1,
	h2,
	h3 {
		font-size: 1.08rem;
		line-height: 1.2;
		letter-spacing: 0;
		margin: 0;
	}

	.compact :is(h1, h2, h3) {
		font-size: 0.98rem;
	}

	.description {
		max-width: 64ch;
		font-size: 0.88rem;
		line-height: 1.5;
		color: #5f6c79;
	}

	.action-link {
		flex: 0 0 auto;
		white-space: nowrap;
		text-decoration: underline;
		text-underline-offset: 0.12em;
		font-size: 0.82rem;
		font-weight: 600;
		color: #6d5a4a;
	}

	@media (max-width: 720px) {
		.section-header {
			flex-direction: column;
			align-items: start;
		}
	}
</style>
