<script lang="ts">
	import type { StellarContractAddress } from '$lib/dashboard/types';

	interface Props {
		contractAddress: StellarContractAddress;
		size?: 'small' | 'medium';
	}

	let { contractAddress, size = 'medium' }: Props = $props();
	let cells = $derived(makePattern(contractAddress));

	function makePattern(address: string): readonly boolean[] {
		const source = address.slice(1);
		const leftHalf = Array.from({ length: 15 }, (_, index) => {
			const character = source.charCodeAt((index * 7) % source.length);
			return ((character + index * 11) & 1) === 1;
		});

		return Array.from({ length: 25 }, (_, index) => {
			const row = Math.floor(index / 5);
			const column = index % 5;
			return leftHalf[row * 3 + Math.min(column, 4 - column)] ?? false;
		});
	}
</script>

<span class={['contract-identicon', `size-${size}`]} aria-hidden="true">
	{#each cells as active, index (`${contractAddress}-${index}`)}
		<span class={['cell', active && 'active']}></span>
	{/each}
</span>

<style>
	.contract-identicon {
		display: inline-grid;
		flex: 0 0 auto;
		grid-template-columns: repeat(5, 1fr);
		box-sizing: border-box;
		border: 1px solid var(--app-rule);
		border-radius: 4px;
		background: var(--app-paper);
	}

	.size-small {
		width: 26px;
		height: 26px;
		padding: 4px;
		gap: 1px;
	}

	.size-medium {
		width: 34px;
		height: 34px;
		padding: 5px;
		gap: 1px;
	}

	.cell {
		background: transparent;
	}

	.cell.active {
		background: var(--app-violet);
	}
</style>
