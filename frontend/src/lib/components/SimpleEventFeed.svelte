<script lang="ts">
	import { resolve } from '$app/paths';
	import { canonicalProtocolSlug, protocolDisplayName, protocolPageHref } from '$lib/catalog';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import SolanaRuntimeIdentity from '$lib/components/SolanaRuntimeIdentity.svelte';
	import { compactLongTokens } from '$lib/event-preview';
	import { readableInstructionName } from '$lib/format';
	import type { ChainEvent, DefiProtocolDescriptor } from '$lib/types';

	let {
		events,
		protocols,
		limit = null,
		title = 'Live feed',
		description = null,
		feedProtocolSlug = null,
		emptyLabel = 'No alerts for this view yet.',
		fallbackProtocolSlug = null,
		fallbackProtocolLabel = null,
		relativeTimestamps = true,
		nowMs = Date.now(),
		showFeedLink = true,
		explorerCluster = 'mainnet-beta'
	}: {
		events: ChainEvent[];
		protocols: DefiProtocolDescriptor[];
		limit?: number | null;
		title?: string;
		description?: string | null;
		feedProtocolSlug?: string | null;
		emptyLabel?: string;
		fallbackProtocolSlug?: string | null;
		fallbackProtocolLabel?: string | null;
		relativeTimestamps?: boolean;
		nowMs?: number;
		showFeedLink?: boolean;
		explorerCluster?: string;
	} = $props();

	let copiedEventId = $state<string | null>(null);
	const visibleEvents = $derived(limit == null ? events : events.slice(0, limit));

	function payloadString(event: ChainEvent, key: string) {
		const value = event.payload[key];
		return typeof value === 'string' && value.length > 0 ? value : null;
	}

	function payloadBool(event: ChainEvent, key: string) {
		const value = event.payload[key];
		return typeof value === 'boolean' ? value : false;
	}

	function eventSeverity(event: ChainEvent) {
		const payloadSeverity = payloadString(event, 'severity');
		const isHidden = payloadSeverity === 'hidden' || payloadString(event, 'classification') === 'hidden';
		const isHiddenAnchorUnknown =
			event.kind === 'program_unknown_instruction' &&
			event.tags.includes('signal:unknown_instruction') &&
			isHidden &&
			payloadString(event, 'instruction_name') === 'legacy_anchor_idl_staging' &&
			payloadString(event, 'discriminator') === '40f4bc78a7e9690a';
		if (
			(!isHidden || isHiddenAnchorUnknown) &&
			(payloadSeverity === 'critical' ||
				event.tags.includes('severity:critical') ||
				event.tags.includes('signal:unknown_instruction'))
		) {
			return 'critical';
		}
		if (
			payloadSeverity === 'high' ||
			event.tags.includes('severity:high') ||
			event.tags.includes('impact:high')
		) {
			return 'high';
		}
		return 'standard';
	}

	function severityLabel(event: ChainEvent) {
		const severity = eventSeverity(event);
		if (severity === 'critical') return 'Critical';
		if (severity === 'high') return 'High';
		return 'Normal';
	}

	function eventTimestamp(event: ChainEvent) {
		return event.block_time ?? event.indexed_at;
	}

	function formatWhen(event: ChainEvent) {
		const timestamp = eventTimestamp(event);
		if (!timestamp) return 'Time unavailable';
		return new Intl.DateTimeFormat('en-US', {
			month: 'short',
			day: 'numeric',
			hour: '2-digit',
			minute: '2-digit'
		}).format(new Date(timestamp * 1000));
	}

	function relativeWhen(event: ChainEvent) {
		const timestamp = eventTimestamp(event);
		if (!timestamp) return 'Time unavailable';
		const seconds = Math.max(0, Math.floor((nowMs - timestamp * 1000) / 1000));
		if (seconds < 45) return 'now';
		if (seconds < 3_600) return `${Math.max(1, Math.floor(seconds / 60))}m ago`;
		if (seconds < 86_400) return `${Math.max(1, Math.floor(seconds / 3_600))}h ago`;
		return formatWhen(event);
	}

	function displayWhen(event: ChainEvent) {
		return relativeTimestamps ? relativeWhen(event) : formatWhen(event);
	}

	function timestampAttribute(event: ChainEvent) {
		const timestamp = eventTimestamp(event);
		return timestamp ? new Date(timestamp * 1000).toISOString() : undefined;
	}

	function eventProtocolSlug(event: ChainEvent) {
		let squadsSlug: string | null = null;
		for (const tag of event.tags) {
			if (!tag.startsWith('protocol:')) continue;
			const slug = tag.slice('protocol:'.length);
			if (slug === 'squads') {
				squadsSlug = slug;
				continue;
			}
			return slug;
		}
		const payloadSlugs = [
			payloadString(event, 'decoded_protocol'),
			payloadString(event, 'protocol_slug'),
			payloadString(event, 'protocol')
		];
		return payloadSlugs.find((slug) => slug && slug !== 'squads') ?? squadsSlug ?? fallbackProtocolSlug;
	}

	function protocolForEvent(event: ChainEvent) {
		const slug = eventProtocolSlug(event);
		const canonicalSlug = slug ? canonicalProtocolSlug(slug) : null;
		const protocol = protocols.find((candidate) => candidate.slug === canonicalSlug);
		if (protocol) return protocol;
		if (fallbackProtocolSlug && fallbackProtocolLabel) {
			return {
				slug: canonicalProtocolSlug(fallbackProtocolSlug),
				name: fallbackProtocolLabel
			};
		}
		return null;
	}

	function protocolHref(event: ChainEvent) {
		const slug = eventProtocolSlug(event);
		return slug ? protocolPageHref(slug) : null;
	}

	function protocolNameForSlug(slug: string | null) {
		if (!slug) return null;
		return protocolDisplayName(protocols, slug);
	}

	function linkedProtocolName(event: ChainEvent) {
		const linked = event.payload.linked_protocols;
		if (Array.isArray(linked)) {
			for (const item of linked) {
				if (!item || typeof item !== 'object') continue;
				const record = item as Record<string, unknown>;
				const slug = typeof record.protocol === 'string' ? record.protocol : null;
				const name = typeof record.protocol_name === 'string' ? record.protocol_name : null;
				if (slug === 'squads') continue;
				return name ?? protocolNameForSlug(slug);
			}
		}

		const decodedProtocol = payloadString(event, 'decoded_protocol');
		if (decodedProtocol && decodedProtocol !== 'squads') {
			return protocolNameForSlug(decodedProtocol) ?? decodedProtocol.replaceAll('_', ' ');
		}

		const protocolSlug = eventProtocolSlug(event);
		if (protocolSlug && protocolSlug !== 'squads') return protocolNameForSlug(protocolSlug);
		return null;
	}

	function instructionLabel(event: ChainEvent) {
		const instruction =
			payloadString(event, 'decoded_instruction_name') ?? payloadString(event, 'instruction_name');
		return instruction ? readableInstructionName(instruction) : null;
	}

	function isSquadsContext(event: ChainEvent) {
		return (
			event.tags.includes('source:squads') ||
			payloadBool(event, 'via_multisig') ||
			payloadString(event, 'call_origin') === 'squads_execution' ||
			payloadString(event, 'protocol') === 'squads'
		);
	}

	function eventTitle(event: ChainEvent) {
		const protocolName = linkedProtocolName(event);
		const instruction = instructionLabel(event);

		if (event.kind === 'multisig_vault_executed') {
			if (protocolName && instruction) return `${protocolName} ${instruction} via Squads`;
			if (protocolName) return `${protocolName} multisig execution`;
			return 'Squads vault execution';
		}

		if (event.kind === 'multisig_proposal_approved') {
			return protocolName ? `${protocolName} multisig approval` : 'Squads approval';
		}
		if (event.kind === 'multisig_proposal_rejected') {
			return protocolName ? `${protocolName} multisig rejection` : 'Squads rejection';
		}
		if (event.kind === 'multisig_proposal_cancelled') {
			return protocolName ? `${protocolName} multisig cancellation` : 'Squads cancellation';
		}
		if (event.kind === 'multisig_proposal_activated') {
			return protocolName ? `${protocolName} multisig proposal` : 'Squads proposal activated';
		}
		if (event.kind === 'multisig_config_executed') {
			return protocolName ? `${protocolName} multisig config change` : 'Squads config change';
		}

		if (event.kind === 'program_admin_instruction' && protocolName && instruction) {
			return isSquadsContext(event)
				? `${protocolName} ${instruction} via Squads`
				: `${protocolName} ${instruction}`;
		}

		return compactLongTokens(event.summary);
	}

	function isBpfLoaderEvent(event: ChainEvent) {
		return (
			event.target_type === 'program' &&
			(event.kind === 'program_deployed' ||
				event.kind === 'program_upgraded' ||
				event.kind === 'program_finalized' ||
				event.kind === 'program_migrated_to_loader_v4' ||
				event.tags.includes('signal:program_update') ||
				payloadString(event, 'loader') !== null)
		);
	}

	function formatCount(value: number) {
		return new Intl.NumberFormat('en-US').format(value);
	}

	function shortSignature(signature: string) {
		if (signature.length <= 16) return signature;
		return `${signature.slice(0, 6)}...${signature.slice(-6)}`;
	}

	function transactionExplorerUrl(signature: string) {
		const clusterQuery = explorerCluster === 'mainnet-beta' ? '' : `?cluster=${explorerCluster}`;
		return `https://explorer.solana.com/tx/${signature}${clusterQuery}`;
	}

	async function writeClipboard(value: string) {
		try {
			await navigator.clipboard.writeText(value);
			return true;
		} catch {
			const textArea = document.createElement('textarea');
			textArea.value = value;
			textArea.setAttribute('readonly', '');
			textArea.style.position = 'fixed';
			textArea.style.top = '0';
			textArea.style.left = '-9999px';
			document.body.appendChild(textArea);
			textArea.select();
			const copied = document.execCommand('copy');
			textArea.remove();
			return copied;
		}
	}

	async function copySignature(event: ChainEvent) {
		if (await writeClipboard(event.signature)) {
			copiedEventId = event.id;
			window.setTimeout(() => {
				if (copiedEventId === event.id) copiedEventId = null;
			}, 1200);
		} else {
			copiedEventId = null;
		}
	}

	function openExplorer(signature: string) {
		window.open(transactionExplorerUrl(signature), '_blank', 'noopener,noreferrer');
	}
</script>

<section class="event-feed-panel" aria-label={title}>
	<div class="feed-head">
		<div>
			<h2>{title}</h2>
			{#if description}
				<p>{description}</p>
			{/if}
		</div>
		{#if showFeedLink}
			{#if feedProtocolSlug}
				<a href={resolve('/protocols/[slug]/activity', { slug: feedProtocolSlug })}>Open feed</a>
			{:else}
				<a href={resolve('/events')}>Open feed</a>
			{/if}
		{/if}
	</div>

	{#if visibleEvents.length === 0}
		<p class="empty-state">{emptyLabel}</p>
	{:else}
		<div class="event-list">
				{#each visibleEvents as event (event.id)}
					{@const protocol = protocolForEvent(event)}
					<article class="event-row">
						<span class={`severity ${eventSeverity(event)}`}>{severityLabel(event)}</span>

						<div class="event-main">
							<div class="event-topline">
								<div class="event-protocol">
									{#if protocol}
										<ProtocolIdentity
											slug={protocol.slug}
											label={protocol.name}
											href={protocolHref(event)}
											size="sm"
										/>
									{:else if isBpfLoaderEvent(event)}
										<SolanaRuntimeIdentity label="BPF Loader" size="sm" />
									{:else}
										<span>Protocol alert</span>
									{/if}
								</div>

								<time datetime={timestampAttribute(event)}>{displayWhen(event)}</time>
							</div>

							<a class="event-link" href={resolve('/events/[id]', { id: event.id })}>{eventTitle(event)}</a>

							<div class="event-meta">
								<span class="slot-label">slot {formatCount(event.slot)}</span>
								<span class="signature-chip">
									<span>Tx</span>
									<code>{shortSignature(event.signature)}</code>
								</span>
								<span class="event-actions">
									<button
										class="event-action"
										type="button"
										onclick={() => copySignature(event)}
										aria-label="Copy transaction signature"
										title={copiedEventId === event.id ? 'Copied' : 'Copy transaction signature'}
									>
										{#if copiedEventId === event.id}
											<svg viewBox="0 0 20 20" aria-hidden="true">
												<path d="M7.7 14.2a.75.75 0 0 1-1.06 0L3.3 10.86A.75.75 0 0 1 4.36 9.8l2.81 2.81 8.47-8.47a.75.75 0 1 1 1.06 1.06z"></path>
											</svg>
										{:else}
											<svg viewBox="0 0 20 20" aria-hidden="true">
												<path d="M7 3.75A2.25 2.25 0 0 1 9.25 1.5h6A2.25 2.25 0 0 1 17.5 3.75v8A2.25 2.25 0 0 1 15.25 14h-6A2.25 2.25 0 0 1 7 11.75zM4.75 6A2.25 2.25 0 0 0 2.5 8.25v8A2.25 2.25 0 0 0 4.75 18.5h6A2.25 2.25 0 0 0 13 16.25V16h-2.25A3.75 3.75 0 0 1 7 12.25V6z"></path>
											</svg>
										{/if}
									</button>
									<button
										class="event-action"
										type="button"
										onclick={() => openExplorer(event.signature)}
										aria-label="Open transaction in explorer"
										title="Open transaction in explorer"
									>
										<svg viewBox="0 0 20 20" aria-hidden="true">
											<path d="M11.75 3.5a.75.75 0 0 0 0 1.5h2.19L8.97 9.97a.75.75 0 1 0 1.06 1.06L15 6.06v2.19a.75.75 0 0 0 1.5 0V3.5z"></path>
											<path d="M5 4.25A2.75 2.75 0 0 0 2.25 7v8A2.75 2.75 0 0 0 5 17.75h8A2.75 2.75 0 0 0 15.75 15v-3a.75.75 0 0 0-1.5 0v3A1.25 1.25 0 0 1 13 16.25H5A1.25 1.25 0 0 1 3.75 15V7A1.25 1.25 0 0 1 5 5.75h3a.75.75 0 0 0 0-1.5z"></path>
										</svg>
									</button>
								</span>
							</div>
						</div>
					</article>
				{/each}
		</div>
	{/if}
</section>

<style>
	.event-feed-panel {
		display: grid;
		gap: 0.8rem;
		padding: 1rem;
		border: 1px solid var(--fw-border);
		border-radius: 8px;
		background: #fff;
	}

	.feed-head {
		display: flex;
		align-items: start;
		justify-content: space-between;
		gap: 0.8rem;
	}

	h2,
	p {
		margin: 0;
	}

	h2 {
		font-size: 1rem;
		line-height: 1.25;
	}

	p,
	.empty-state {
		color: var(--fw-muted);
		font-size: 0.87rem;
		line-height: 1.45;
	}

	.feed-head a {
		flex: 0 0 auto;
		color: #4d4036;
		font-size: 0.82rem;
		font-weight: 760;
		text-decoration: none;
	}

	.feed-head a:hover {
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

		.event-list {
			display: grid;
			border-top: 1px solid rgba(23, 32, 42, 0.08);
		}

		.event-row {
			display: grid;
			grid-template-columns: auto minmax(0, 1fr);
			align-items: start;
			gap: 0.75rem;
			padding: 0.82rem 0;
			border-bottom: 1px solid rgba(23, 32, 42, 0.08);
			color: inherit;
		}

		.event-main {
			display: grid;
			gap: 0.42rem;
			min-width: 0;
		}

		.event-topline,
		.event-protocol,
		.event-meta {
			display: flex;
			align-items: center;
			min-width: 0;
		}

	.event-topline {
		justify-content: space-between;
		gap: 0.7rem;
	}

	.event-protocol {
		gap: 0.46rem;
		min-width: 0;
	}

	time {
		flex: 0 0 auto;
		color: #6d7884;
		font-size: 0.76rem;
		font-weight: 720;
		line-height: 1.25;
	}

	.event-link {
		color: #1b242c;
		font-size: 0.91rem;
		font-weight: 760;
		line-height: 1.38;
		text-decoration: none;
		overflow-wrap: anywhere;
	}

	.event-link:hover {
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

		.severity {
			justify-self: start;
			width: max-content;
			max-width: 100%;
			margin-top: 0.06rem;
			padding: 0.18rem 0.42rem;
			border-radius: 6px;
			border: 1px solid rgba(23, 32, 42, 0.1);
			background: #f7f6f2;
			color: #4f5d54;
			font-size: 0.72rem;
			font-weight: 780;
			line-height: 1.2;
			text-align: center;
		}

	.severity.critical {
		border-color: rgba(156, 57, 41, 0.2);
		background: rgba(156, 57, 41, 0.08);
		color: #9c3929;
	}

	.severity.high {
		border-color: rgba(138, 94, 30, 0.22);
		background: rgba(138, 94, 30, 0.08);
		color: #7a4e17;
	}

	.event-meta {
		flex-wrap: wrap;
		gap: 0.32rem 0.5rem;
		color: var(--fw-muted);
		font-size: 0.74rem;
		font-weight: 650;
	}

	.slot-label {
		color: #7a8793;
		font-size: 0.69rem;
		font-weight: 680;
	}

	.signature-chip {
		display: inline-flex;
		align-items: center;
		gap: 0.32rem;
		min-width: 0;
	}

	.signature-chip span {
		color: #7a8793;
		font-size: 0.69rem;
		font-weight: 700;
	}

		.signature-chip code {
			color: #475462;
			font-size: 0.72rem;
		}

		.event-actions {
			display: inline-flex;
			align-items: center;
			gap: 0.32rem;
			flex: 0 0 auto;
		}

		.event-action {
			display: inline-flex;
			align-items: center;
			justify-content: center;
			flex: 0 0 auto;
			width: 1.58rem;
			height: 1.58rem;
			padding: 0;
			border: 1px solid rgba(23, 32, 42, 0.1);
			border-radius: 6px;
			background: #fff;
			color: #4d5966;
			cursor: pointer;
		}

		.event-action svg {
			width: 0.88rem;
			height: 0.88rem;
			fill: currentColor;
		}

		.event-action:hover {
			border-color: rgba(173, 78, 34, 0.28);
			color: #17202a;
		}

	.empty-state {
		padding: 0.74rem 0;
	}

	@media (max-width: 560px) {
			.feed-head {
				display: grid;
				grid-template-columns: 1fr;
			}

			.event-row {
				grid-template-columns: auto minmax(0, 1fr);
				gap: 0.56rem;
			}

			.event-topline {
			align-items: flex-start;
			gap: 0.52rem;
		}

			.event-protocol {
				flex-wrap: wrap;
			}

			time {
				padding-top: 0.08rem;
			}

		.event-meta {
			gap: 0.3rem 0.42rem;
		}
	}
</style>
