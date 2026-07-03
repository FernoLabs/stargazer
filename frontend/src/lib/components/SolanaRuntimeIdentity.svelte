<script lang="ts">
	let {
		label = 'BPF Loader',
		size = 'sm'
	}: {
		label?: string;
		size?: 'sm' | 'md';
	} = $props();
</script>

<span class:small={size === 'sm'} class="runtime-identity">
	<svg class="solana-mark" viewBox="0 0 397.7 311.7" aria-hidden="true" focusable="false">
		<defs>
			<linearGradient id="solana-a" x1="360.9" x2="141.2" y1="351.5" y2="-69.3" gradientUnits="userSpaceOnUse">
				<stop offset="0" stop-color="#00FFA3" />
				<stop offset="1" stop-color="#DC1FFF" />
			</linearGradient>
			<linearGradient id="solana-b" x1="264.8" x2="45.1" y1="401.7" y2="-19.1" gradientUnits="userSpaceOnUse">
				<stop offset="0" stop-color="#00FFA3" />
				<stop offset="1" stop-color="#DC1FFF" />
			</linearGradient>
			<linearGradient id="solana-c" x1="312.5" x2="92.8" y1="376.8" y2="-44" gradientUnits="userSpaceOnUse">
				<stop offset="0" stop-color="#00FFA3" />
				<stop offset="1" stop-color="#DC1FFF" />
			</linearGradient>
		</defs>
		<path fill="url(#solana-a)" d="M64.6 237.9c2.4-2.4 5.7-3.8 9.2-3.8h317.4c5.8 0 8.7 7 4.6 11.1l-62.7 62.7c-2.4 2.4-5.7 3.8-9.2 3.8H6.5c-5.8 0-8.7-7-4.6-11.1z" />
		<path fill="url(#solana-b)" d="M64.6 3.8C67.1 1.4 70.4 0 73.8 0h317.4c5.8 0 8.7 7 4.6 11.1l-62.7 62.7c-2.4 2.4-5.7 3.8-9.2 3.8H6.5c-5.8 0-8.7-7-4.6-11.1z" />
		<path fill="url(#solana-c)" d="M333.1 120.1c-2.4-2.4-5.7-3.8-9.2-3.8H6.5c-5.8 0-8.7 7-4.6 11.1l62.7 62.7c2.4 2.4 5.7 3.8 9.2 3.8h317.4c5.8 0 8.7-7 4.6-11.1z" />
	</svg>
	<span>{label}</span>
</span>

<style>
	.runtime-identity {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
		min-width: 0;
		color: inherit;
	}

	.runtime-identity.small {
		gap: 0.38rem;
	}

	.solana-mark {
		width: 1.25rem;
		height: 1.25rem;
		flex: 0 0 auto;
	}

	.runtime-identity.small .solana-mark {
		width: 1rem;
		height: 1rem;
	}

	span span {
		min-width: 0;
		font-size: 0.85rem;
		font-weight: 700;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.runtime-identity.small span {
		font-size: 0.74rem;
	}
</style>
