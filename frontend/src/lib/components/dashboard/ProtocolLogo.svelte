<script lang="ts">
	import { protocolBrand, protocolInitials } from '$lib/dashboard/protocols';

	interface Props {
		protocolId: string;
		size?: 'small' | 'medium' | 'large';
	}

	let { protocolId, size = 'medium' }: Props = $props();
	let failedPath = $state<string | null>(null);
	let brand = $derived(protocolBrand(protocolId));
	let showImage = $derived(Boolean(brand.logoPath) && failedPath !== brand.logoPath);

	function markFailed() {
		failedPath = brand.logoPath ?? null;
	}
</script>

<span
	class={['protocol-logo', `size-${size}`, `surface-${brand.logoSurface}`]}
	aria-hidden="true"
>
	{#if showImage && brand.logoPath}
		<img src={brand.logoPath} alt="" loading="lazy" decoding="async" onerror={markFailed} />
	{:else}
		<span>{protocolInitials(protocolId)}</span>
	{/if}
</span>

<style>
	.protocol-logo {
		display: inline-grid;
		flex: 0 0 auto;
		place-items: center;
		box-sizing: border-box;
		border: 1px solid var(--app-rule);
		border-radius: 4px;
		background: var(--app-paper);
		overflow: hidden;
	}

	.size-small {
		width: 26px;
		height: 26px;
		padding: 3px;
	}

	.size-medium {
		width: 34px;
		height: 34px;
		padding: 4px;
	}

	.size-large {
		width: 42px;
		height: 42px;
		padding: 5px;
	}

	.surface-dark {
		border-color: #242229;
		background: #242229;
	}

	img {
		display: block;
		width: 100%;
		height: 100%;
		object-fit: contain;
	}

	.protocol-logo > span {
		color: var(--app-violet);
		font: 700 0.58rem/1 'Fragment Mono', monospace;
		letter-spacing: -0.04em;
	}

	.size-large > span {
		font-size: 0.68rem;
	}
</style>
