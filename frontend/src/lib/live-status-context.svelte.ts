import { getContext, setContext } from 'svelte';
import type { LiveStatusResponse } from '$lib/types';

const SLOT_TICK_MS = 1000;
const LIVE_STATUS_CONTEXT = Symbol('live-status');

export class LiveStatusContext {
	liveStatus = $state<LiveStatusResponse | null>(null);
	liveStatusError = $state<string | null>(null);
	estimatedNowMs = $state(Date.now());
	telegramSubscriptionCount = $state(0);

	constructor(
		liveStatus: () => LiveStatusResponse | null,
		telegramSubscriptionCount: () => number
	) {
		this.liveStatus = liveStatus();
		this.telegramSubscriptionCount = telegramSubscriptionCount();
	}

	summary() {
		return this.liveStatus?.summary ?? null;
	}

	estimatedLiveSlot() {
		const baseSlot = this.liveStatus?.summary.last_processed_slot;
		if (baseSlot == null) return null;

		const sampledAtMs = this.liveStatus?.sampled_at_ms;
		const slotTimeMs = Math.max(1, this.liveStatus?.estimated_slot_time_ms ?? SLOT_TICK_MS);
		if (!sampledAtMs) return baseSlot;

		const elapsedMs = Math.max(0, this.estimatedNowMs - sampledAtMs);
		return baseSlot + Math.floor(elapsedMs / slotTimeMs);
	}

	indexingStartSlot() {
		const summary = this.summary();
		return summary?.backfill_start_slot ?? summary?.first_indexed_slot ?? null;
	}
}

export function createLiveStatusContext(
	liveStatus: () => LiveStatusResponse | null,
	telegramSubscriptionCount: () => number
) {
	const context = new LiveStatusContext(liveStatus, telegramSubscriptionCount);
	setContext(LIVE_STATUS_CONTEXT, context);
	return context;
}

export function getLiveStatusContext() {
	return getContext<LiveStatusContext>(LIVE_STATUS_CONTEXT);
}
