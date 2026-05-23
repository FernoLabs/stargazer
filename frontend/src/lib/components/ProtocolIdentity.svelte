<script lang="ts">
	import {
		protocolLogoUrl,
		protocolMonogram,
		protocolTone
	} from '$lib/protocol-brand';

	let {
		slug,
		label,
		href = null,
		size = 'md'
	}: {
		slug: string | null;
		label: string;
		href?: string | null;
		size?: 'sm' | 'md';
	} = $props();

	let logoFailed = $state(false);

	function logoUrl() {
		return protocolLogoUrl(slug, logoFailed ? { [slug ?? '']: true } : {});
	}
</script>

<svelte:element this={href ? 'a' : 'span'} class:small={size === 'sm'} class={`protocol-identity tone-${protocolTone(slug)}`} href={href ?? undefined}>
	{#if logoUrl()}
		<img class="protocol-logo" src={logoUrl() ?? ''} alt="" loading="lazy" decoding="async" onerror={() => (logoFailed = true)} />
	{:else}
		<span class="protocol-avatar">{protocolMonogram(slug, label)}</span>
	{/if}
	<span class="protocol-label">{label}</span>
</svelte:element>

<style>
	.protocol-identity {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
		color: inherit;
		text-decoration: none;
		min-width: 0;
	}

	.protocol-identity.small {
		gap: 0.38rem;
	}

	.protocol-avatar,
	.protocol-logo {
		width: 1.25rem;
		height: 1.25rem;
		border-radius: 6px;
		flex: 0 0 auto;
	}

	.protocol-identity.small .protocol-avatar,
	.protocol-identity.small .protocol-logo {
		width: 1rem;
		height: 1rem;
	}

		.protocol-avatar {
			display: inline-flex;
			align-items: center;
			justify-content: center;
			font-size: 0.62rem;
			font-weight: 800;
			letter-spacing: 0;
			color: #14202a;
		}

	.protocol-label {
		min-width: 0;
		font-size: 0.85rem;
		font-weight: 700;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.protocol-identity.small .protocol-label {
		font-size: 0.74rem;
	}

	.tone-amber .protocol-avatar {
		background: rgba(255, 190, 117, 0.3);
	}

	.tone-teal .protocol-avatar {
		background: rgba(79, 214, 193, 0.22);
	}

	.tone-ocean .protocol-avatar {
		background: rgba(117, 180, 255, 0.22);
	}

	.tone-red .protocol-avatar {
		background: rgba(255, 123, 112, 0.22);
	}

	.tone-forest .protocol-avatar {
		background: rgba(104, 181, 118, 0.22);
	}

	.tone-indigo .protocol-avatar {
		background: rgba(140, 124, 255, 0.2);
	}
</style>
