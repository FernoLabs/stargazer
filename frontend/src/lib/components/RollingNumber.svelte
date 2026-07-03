<script lang="ts">
	import { onDestroy } from 'svelte';

	type Props = {
		value: number | null | undefined;
		durationMs?: number;
	};

	const numberFormatter = new Intl.NumberFormat('en-US');

	let { value, durationMs = 800 }: Props = $props();
	let displayValue = $state<number | null>(null);
	let reducedMotion = $state(false);
	let frame: number | null = null;
	const targetValue = $derived(normalizeNumber(value));
	const visibleValue = $derived(displayValue ?? targetValue);

	function normalizeNumber(next: number | null | undefined) {
		if (next == null || !Number.isFinite(next)) return 0;
		return Math.max(0, next);
	}

	function formatNumber(next: number) {
		return numberFormatter.format(Math.round(next));
	}

	function stopAnimation() {
		if (frame == null) return;
		window.cancelAnimationFrame(frame);
		frame = null;
	}

	$effect(() => {
		const media = window.matchMedia('(prefers-reduced-motion: reduce)');
		const updatePreference = () => {
			reducedMotion = media.matches;
		};

		updatePreference();
		media.addEventListener('change', updatePreference);

		return () => {
			media.removeEventListener('change', updatePreference);
		};
	});

	$effect(() => {
		const target = targetValue;
		stopAnimation();

		const start = displayValue ?? target;
		if (reducedMotion || durationMs <= 0 || Math.abs(target - start) < 1) {
			displayValue = target;
			return;
		}

		const delta = target - start;
		const startedAt = window.performance.now();
		const duration = Math.max(120, durationMs);

		function tick(now: number) {
			const progress = Math.min(1, (now - startedAt) / duration);
			const eased = 1 - Math.pow(1 - progress, 3);
			displayValue = start + delta * eased;

			if (progress < 1) {
				frame = window.requestAnimationFrame(tick);
			} else {
				displayValue = target;
				frame = null;
			}
		}

		frame = window.requestAnimationFrame(tick);

		return stopAnimation;
	});

	onDestroy(stopAnimation);
</script>

<span class="rolling-number" aria-live="polite" aria-label={formatNumber(targetValue)}>
	{formatNumber(visibleValue)}
</span>

<style>
	.rolling-number {
		display: inline-block;
		font-variant-numeric: tabular-nums;
	}
</style>
