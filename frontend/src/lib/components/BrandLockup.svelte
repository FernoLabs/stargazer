<script lang="ts">
	import BrandMark from '$lib/components/BrandMark.svelte';

	let {
		href = '/',
		compact = false,
		showByline = false
	}: {
		href?: string;
		compact?: boolean;
		showByline?: boolean;
	} = $props();
</script>

<a class:compact class="brand-lockup" href={href} aria-label="FireWatch home">
	<BrandMark size={compact ? 34 : 40} />
	<span class="brand-copy">
		<strong>FireWatch</strong>
		{#if showByline}
			<span>Protocol safety</span>
		{/if}
	</span>
</a>

<style>
	.brand-lockup {
		display: inline-flex;
		align-items: center;
		gap: 0.7rem;
		text-decoration: none;
		color: inherit;
		min-width: 0;
	}

	.brand-copy {
		display: grid;
		gap: 0.08rem;
		min-width: 0;
	}

	strong {
		font-size: 1.03rem;
		line-height: 1;
		letter-spacing: 0;
	}

	.brand-copy > span {
		font-size: 0.74rem;
		font-weight: 600;
		color: #6e7782;
	}

	.compact strong {
		font-size: 0.98rem;
	}

	.compact .brand-copy > span {
		font-size: 0.72rem;
	}
</style>
