<script lang="ts">
	import type { LiveStatusContext } from '$lib/live-status-context.svelte';

	let { status }: { status: LiveStatusContext } = $props();
	const formatter = new Intl.NumberFormat('en-US');

	function formatCount(value: number | null | undefined) {
		return formatter.format(value ?? 0);
	}
</script>

<div class="live-status-strip" aria-label="Live status">
	<span class="live-pill tip-pill">
		<span class="tip-main">
			<span>Tip</span>
			<strong>{formatCount(status.estimatedLiveSlot())}</strong>
		</span>
		{#if status.indexingStartSlot() != null}
			<span class="tip-start">start {formatCount(status.indexingStartSlot())}</span>
		{/if}
	</span>
	<span class="live-pill">
		<span>Events</span>
		<strong>{formatCount(status.summary()?.total_count)}</strong>
	</span>
	<span class="live-pill">
		<span>Subs</span>
		<strong>{formatCount(status.telegramSubscriptionCount)}</strong>
	</span>
</div>

<style>
	.live-status-strip {
		display: flex;
		flex-wrap: nowrap;
		gap: 0;
		min-width: max-content;
	}

	.live-pill {
		display: inline-flex;
		align-items: baseline;
		gap: 0.34rem;
		min-width: 0;
		padding: 0.1rem 0 0.1rem 0.8rem;
		margin-left: 0.8rem;
		border-left: 1px solid rgba(23, 32, 42, 0.1);
	}

	.live-pill:first-child {
		margin-left: 0;
		padding-left: 0;
		border-left: none;
	}

	.tip-pill {
		display: grid;
		align-items: center;
		gap: 0.02rem;
	}

	.tip-main {
		display: inline-flex;
		align-items: baseline;
		gap: 0.34rem;
	}

	.live-pill span {
		font-size: 0.76rem;
		font-weight: 600;
		color: #677584;
	}

	.live-pill strong {
		font-size: 0.82rem;
		white-space: nowrap;
	}

	.tip-start {
		justify-self: end;
		font-size: 0.64rem;
		line-height: 1;
		color: #8a7666;
		white-space: nowrap;
	}

	@media (max-width: 560px) {
		.live-pill {
			flex: 0 0 auto;
			gap: 0.28rem;
			margin-left: 0;
			padding: 0 0.62rem 0 0;
			border-left: none;
			border-right: 1px solid rgba(23, 32, 42, 0.1);
		}

		.live-pill:first-child {
			padding-left: 0;
		}

		.live-pill:last-child {
			padding-right: 0;
			border-right: none;
		}

		.live-pill span {
			font-size: 0.72rem;
		}

		.live-pill strong {
			font-size: 0.78rem;
		}

		.tip-pill,
		.tip-main {
			display: inline-flex;
			align-items: baseline;
		}

		.tip-start {
			display: none;
		}
	}
</style>
