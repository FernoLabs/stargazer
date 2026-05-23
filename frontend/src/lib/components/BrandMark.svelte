<script lang="ts">
	import towerIcon from '$lib/assets/firewatch-tower-icon.png';

	let { size = 40 }: { size?: number } = $props();
</script>

<span class="brand-mark" style={`width:${size}px;height:${size}px;`}>
	<img src={towerIcon} alt="" aria-hidden="true" />
</span>

<style>
	.brand-mark {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex: 0 0 auto;
	}

	img {
		display: block;
		width: 100%;
		height: 100%;
	}
</style>
