<script lang="ts">
	import type { LiveStatusContext } from '$lib/live-status-context.svelte';

	let { status }: { status: LiveStatusContext } = $props();

	function statusLabel() {
		if (status.liveStatusError && !status.liveStatus) return 'Live indexer delayed';
		if (status.summary()?.last_processed_slot != null) return 'Live indexer online';
		return 'Live indexer connecting';
	}

	function updateLabel() {
		const sampledAtMs = status.liveStatus?.sampled_at_ms;
		if (!sampledAtMs) return 'Waiting for first update';
		const seconds = Math.max(0, Math.floor((status.estimatedNowMs - sampledAtMs) / 1000));
		if (seconds <= 5) return 'Updated now';
		if (seconds < 60) return `Updated ${seconds}s ago`;
		return `Updated ${Math.floor(seconds / 60)}m ago`;
	}
</script>

<div class="live-status-strip" aria-label="Live status">
	<strong>{statusLabel()}</strong>
	<span>{updateLabel()}</span>
</div>

<style>
	.live-status-strip {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		min-width: 0;
		white-space: nowrap;
	}

	.live-status-strip strong {
		color: #167044;
		font-size: 0.82rem;
		font-weight: 800;
	}

	.live-status-strip span {
		color: #65717e;
		font-size: 0.78rem;
		font-weight: 680;
	}

	@media (max-width: 560px) {
		.live-status-strip {
			width: 100%;
			justify-content: space-between;
		}
	}
</style>
