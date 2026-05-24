<script lang="ts">
	import { explorerAddressUrl, explorerTransactionUrl } from '$lib/explorer';
	import { eventDetailHref } from '$lib/event-preview';
	import {
		findProgramMatch,
		programPageHref,
		protocolDisplayName,
		protocolPageHref
	} from '$lib/catalog';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import { formatTimestamp, readableInstructionName, shortAddress } from '$lib/format';
	import type { ChainEvent, DefiProtocolDescriptor } from '$lib/types';

	let {
		event,
		explorerCluster,
		protocols = [],
		activeTag = '',
		onTagSelect = null,
		fallbackProtocolSlug = null,
		fallbackProtocolLabel = null,
		linkTitle = true
	}: {
		event: ChainEvent;
		explorerCluster: string;
		protocols?: DefiProtocolDescriptor[];
		activeTag?: string;
		onTagSelect?: ((tag: string) => void) | null;
		fallbackProtocolSlug?: string | null;
		fallbackProtocolLabel?: string | null;
		linkTitle?: boolean;
	} = $props();

	let copied = $state(false);

	function payloadString(key: string) {
		const value = event.payload[key];
		return typeof value === 'string' && value.length > 0 ? value : null;
	}

	function payloadNumber(key: string) {
		const value = event.payload[key];
		return typeof value === 'number' ? value : null;
	}

	function isCriticalEvent() {
		return (
			event.tags.includes('severity:critical') || event.tags.includes('signal:unknown_instruction')
		);
	}

	function severity() {
		if (isCriticalEvent()) return 'critical';
		if (event.tags.includes('severity:high') || event.tags.includes('impact:high')) return 'high';
		return 'standard';
	}

	function priorityLabel() {
		if (severity() === 'critical') return 'Review now';
		if (severity() === 'high') return 'High';
		return 'Standard';
	}

	function kindLabel() {
		switch (event.kind) {
			case 'name_service_domain_created':
				return 'Domain create';
			case 'name_service_domain_updated':
				return 'Domain update';
			case 'name_service_domain_transferred':
				return 'Domain transfer';
			case 'name_service_domain_deleted':
				return 'Domain delete';
			case 'name_service_domain_reallocated':
				return 'Domain resize';
			case 'program_unknown_instruction':
				return 'Unknown';
			case 'program_admin_instruction':
				return 'Admin';
			case 'program_upgraded':
				return 'Upgrade';
			case 'program_deployed':
				return 'Deploy';
			case 'program_finalized':
				return 'Finalize';
			case 'program_migrated_to_loader_v4':
				return 'Loader';
			case 'program_metric_alert':
				return 'Metric';
			case 'program_idl_initialized':
			case 'program_idl_updated':
			case 'program_idl_authority_changed':
			case 'program_idl_immutable':
			case 'program_idl_resized':
			case 'program_idl_closed':
				return 'IDL';
			case 'multisig_proposal_activated':
				return 'Activated';
			case 'multisig_proposal_approved':
				return 'Approval';
			case 'multisig_proposal_rejected':
				return 'Rejected';
			case 'multisig_proposal_cancelled':
				return 'Cancelled';
			case 'multisig_vault_executed':
				return 'Execution';
			case 'multisig_config_executed':
				return 'Config';
			default:
				return event.kind.replaceAll('_', ' ');
		}
	}

	function protocolSlug() {
		const tag = event.tags.find((candidate) => candidate.startsWith('protocol:'));
		if (tag) return tag.replace('protocol:', '');
		return fallbackProtocolSlug;
	}

	function protocolLabel() {
		const slug = protocolSlug();
		if (!slug) return fallbackProtocolLabel;
		return protocolDisplayName(protocols, slug);
	}

	function eventProtocolHref() {
		const slug = protocolSlug();
		return slug ? protocolPageHref(slug) : null;
	}

	function knownProgramMatch() {
		return findProgramMatch(protocols, event.target_address);
	}

	function targetDisplayName() {
		const domainName = payloadString('domain_name') ?? payloadString('target_name');
		if (domainName) return domainName;

		const match = knownProgramMatch();
		if (match) {
			const protocol = match.protocol.name;
			const program = match.program.label;
			if (protocol.toLowerCase().includes(program.toLowerCase())) return protocol;
			if (program.toLowerCase() === 'core') return protocol;
			return `${protocol} ${program}`;
		}

		const payloadLabel = payloadString('program_label');
		if (payloadLabel) return payloadLabel;

		return shortAddress(event.target_address);
	}

	function eventTitle() {
		const protocol = payloadString('protocol_name') ?? protocolLabel();
		const instruction = payloadString('instruction_name');
		const targetName = targetDisplayName();

		if (event.kind === 'name_service_domain_created') return `${targetName} created`;
		if (event.kind === 'name_service_domain_updated') return `${targetName} updated`;
		if (event.kind === 'name_service_domain_transferred') return `${targetName} ownership changed`;
		if (event.kind === 'name_service_domain_deleted') return `${targetName} deleted`;
		if (event.kind === 'name_service_domain_reallocated') return `${targetName} resized`;
		if (event.kind === 'program_unknown_instruction' && protocol) {
			return `${protocol} unknown instruction`;
		}
		if (event.kind === 'program_upgraded') return `${targetName} upgraded`;
		if (event.kind === 'program_deployed') return `${targetName} deployed`;
		if (event.kind === 'program_finalized') return `${targetName} finalized`;
		if (event.kind === 'program_migrated_to_loader_v4') return `${targetName} migrated to loader v4`;
		if (event.kind === 'program_admin_instruction' && protocol && instruction) {
			return `${protocol} ${readableInstructionName(instruction)}`;
		}
		if (event.kind === 'program_metric_alert' && protocol) {
			return `${protocol} metric anomaly`;
		}
		if (event.kind === 'program_idl_updated' && protocol) return `${protocol} IDL updated`;
		if (event.kind === 'program_idl_initialized' && protocol) return `${protocol} IDL initialized`;
		if (event.kind === 'program_idl_authority_changed' && protocol) return `${protocol} IDL authority changed`;
		if (event.kind === 'program_idl_immutable' && protocol) return `${protocol} IDL immutable`;
		if (event.kind === 'program_idl_resized' && protocol) return `${protocol} IDL resized`;
		if (event.kind === 'program_idl_closed' && protocol) return `${protocol} IDL closed`;
		if (event.kind === 'multisig_proposal_activated') return 'Squads proposal activated';
		if (event.kind === 'multisig_proposal_approved') return 'Squads proposal approval';
		if (event.kind === 'multisig_proposal_rejected') return 'Squads proposal rejected';
		if (event.kind === 'multisig_proposal_cancelled') return 'Squads proposal cancelled';
		if (event.kind === 'multisig_vault_executed') return 'Squads vault execution';
		if (event.kind === 'multisig_config_executed') return 'Squads config execution';
		return event.summary;
	}

	function detailItems() {
		const items: Array<{ label: string; value: string }> = [];
		const instruction = payloadString('instruction_name');
		const callOrigin = payloadString('call_origin');
		const metricLabel = payloadString('metric_label');
		const metricValue = payloadNumber('metric_value');
		const proposal = payloadString('proposal');

		if (instruction) items.push({ label: 'Instruction', value: readableInstructionName(instruction) });
		if (event.tags.includes('signal:idl_update'))
			items.push({ label: 'Reason', value: 'Canonical IDL changed' });
		if (event.tags.includes('signal:unknown_instruction'))
			items.push({ label: 'Reason', value: 'Not in current catalog' });
		if (metricLabel && metricValue !== null)
			items.push({ label: 'Metric', value: `${metricLabel} ${metricValue}` });
		if (callOrigin === 'routed_cpi') items.push({ label: 'Context', value: 'Routed CPI' });
		if (callOrigin === 'self_cpi') items.push({ label: 'Context', value: 'Same-program helper' });
		if (proposal) items.push({ label: 'Proposal', value: shortAddress(proposal) });
		if (event.target_type === 'multisig' && !proposal) items.push({ label: 'Context', value: 'Squads multisig' });
		return items.slice(0, 3);
	}

	function keyFacts() {
		const items: Array<{
			label: string;
			value: string;
			secondary?: string | null;
			url: string;
			internalUrl?: string | null;
		}> = [];

		if (event.target_type === 'multisig') {
			const multisig = payloadString('multisig') ?? event.target_address;
			items.push({
				label: 'Multisig',
				value: payloadString('multisig_name') ?? shortAddress(multisig),
				secondary: shortAddress(multisig),
				url: explorerAddressUrl(multisig, explorerCluster)
			});

			const member = payloadString('member') ?? event.actor;
			if (member) {
				items.push({
					label: 'Member',
					value: payloadString('member_name') ?? payloadString('actor_name') ?? shortAddress(member),
					secondary: shortAddress(member),
					url: explorerAddressUrl(member, explorerCluster)
				});
			}
		} else if (event.target_type === 'domain') {
			items.push({
				label: 'Domain',
				value: targetDisplayName(),
				secondary: shortAddress(event.target_address),
				url: explorerAddressUrl(event.target_address, explorerCluster)
			});

			const owner = payloadString('owner') ?? event.actor;
			if (owner) {
				items.push({
					label: 'Owner',
					value: payloadString('owner_name') ?? payloadString('actor_name') ?? shortAddress(owner),
					secondary: shortAddress(owner),
					url: explorerAddressUrl(owner, explorerCluster)
				});
			}
		} else {
			items.push({
				label: 'Program',
				value: targetDisplayName(),
				secondary: shortAddress(event.target_address),
				url: explorerAddressUrl(event.target_address, explorerCluster),
				internalUrl: programPageHref(event.target_address)
			});

			if (event.actor) {
				items.push({
					label: 'Actor',
					value: payloadString('actor_name') ?? shortAddress(event.actor),
					secondary: shortAddress(event.actor),
					url: explorerAddressUrl(event.actor, explorerCluster)
				});
			}
		}

		const metadataAccount = payloadString('metadata_account');
		if (metadataAccount) {
			items.push({
				label: 'IDL PDA',
				value: shortAddress(metadataAccount),
				url: explorerAddressUrl(metadataAccount, explorerCluster)
			});
		}

		return items;
	}

	function importantTags() {
		return event.tags
			.filter(
				(tag) =>
					!tag.startsWith('target:') &&
					!tag.startsWith('protocol:') &&
					!tag.startsWith('instruction:') &&
					!tag.startsWith('severity:') &&
					!tag.startsWith('source:') &&
					!tag.startsWith('impact:') &&
					tag !== 'watch:defi' &&
					tag !== 'signal:multisig' &&
					tag !== 'signal:program_update'
			)
			.slice(0, 3);
	}

	function shortSignature(signature: string) {
		return `${signature.slice(0, 6)}...${signature.slice(-6)}`;
	}

	async function copySignature() {
		try {
			await navigator.clipboard.writeText(event.signature);
			copied = true;
			window.setTimeout(() => {
				copied = false;
			}, 1200);
		} catch {
			copied = false;
		}
	}

	function visibleFacts() {
		return keyFacts().slice(0, 2);
	}

	function factHref(fact: ReturnType<typeof keyFacts>[number]) {
		return fact.internalUrl ?? fact.url;
	}

	function factTarget(fact: ReturnType<typeof keyFacts>[number]) {
		return fact.internalUrl ? undefined : '_blank';
	}

	function factRel(fact: ReturnType<typeof keyFacts>[number]) {
		return fact.internalUrl ? undefined : 'noreferrer';
	}
</script>

<article class:critical={severity() === 'critical'} class:event-high={severity() === 'high'} class="event-row">
	<div class="event-content">
		<div class="event-top">
			<div class="title-stack">
				<div class="label-row">
					{#if severity() !== 'standard'}
						<span class:priority-critical={severity() === 'critical'} class:priority-high={severity() === 'high'} class="priority-label">
							{priorityLabel()}
						</span>
					{/if}
					<span class="kind-label">{kindLabel()}</span>
					{#if protocolLabel() && eventProtocolHref()}
						<ProtocolIdentity slug={protocolSlug()} label={protocolLabel() ?? ''} href={eventProtocolHref()} size="sm" />
					{:else if protocolLabel()}
						<ProtocolIdentity slug={protocolSlug()} label={protocolLabel() ?? ''} size="sm" />
					{/if}
				</div>

				<h3>
					{#if linkTitle}
						<a href={eventDetailHref(event)}>{eventTitle()}</a>
					{:else}
						{eventTitle()}
					{/if}
				</h3>
			</div>

			<div class="meta-row">
				<span>{formatTimestamp(event.block_time ?? event.indexed_at)}</span>
				<span>slot {event.slot}</span>
				<span>path {event.instruction_path}</span>
			</div>
		</div>

		{#if detailItems().length > 0 || visibleFacts().length > 0}
			<div class="detail-strip">
				{#each visibleFacts() as fact}
					<a class="detail-token fact-token" href={factHref(fact)} rel={factRel(fact)} target={factTarget(fact)}>
						<span>{fact.label}</span>
						<strong>{fact.value}</strong>
						{#if fact.secondary && fact.secondary !== fact.value}
							<code>{fact.secondary}</code>
						{/if}
					</a>
				{/each}

				{#each detailItems() as item}
					<span class="detail-token">
						<span>{item.label}</span>
						<strong>{item.value}</strong>
					</span>
				{/each}
			</div>
		{/if}

		<div class="footer-row">
			<div class="tx-group">
				<div class="signature-chip">
					<span>Tx</span>
					<code>{shortSignature(event.signature)}</code>
				</div>

				<button
					class="icon-button"
					type="button"
					onclick={copySignature}
					aria-label="Copy transaction signature"
					title={copied ? 'Copied' : 'Copy transaction signature'}
				>
					{#if copied}
						✓
					{:else}
						<svg viewBox="0 0 20 20" aria-hidden="true">
							<path d="M7 3.75A2.25 2.25 0 0 1 9.25 1.5h6A2.25 2.25 0 0 1 17.5 3.75v8A2.25 2.25 0 0 1 15.25 14h-6A2.25 2.25 0 0 1 7 11.75zM4.75 6A2.25 2.25 0 0 0 2.5 8.25v8A2.25 2.25 0 0 0 4.75 18.5h6A2.25 2.25 0 0 0 13 16.25V16h-2.25A3.75 3.75 0 0 1 7 12.25V6z"></path>
						</svg>
					{/if}
				</button>
				<a
					class="icon-link"
					href={explorerTransactionUrl(event.signature, explorerCluster)}
					rel="noreferrer"
					target="_blank"
					aria-label="Open transaction in explorer"
					title="Open transaction in explorer"
				>
					<svg viewBox="0 0 20 20" aria-hidden="true">
						<path d="M11.75 3.5a.75.75 0 0 0 0 1.5h2.19L8.97 9.97a.75.75 0 1 0 1.06 1.06L15 6.06v2.19a.75.75 0 0 0 1.5 0V3.5z"></path>
						<path d="M5 4.25A2.75 2.75 0 0 0 2.25 7v8A2.75 2.75 0 0 0 5 17.75h8A2.75 2.75 0 0 0 15.75 15v-3a.75.75 0 0 0-1.5 0v3A1.25 1.25 0 0 1 13 16.25H5A1.25 1.25 0 0 1 3.75 15V7A1.25 1.25 0 0 1 5 5.75h3a.75.75 0 0 0 0-1.5z"></path>
					</svg>
				</a>
			</div>

			{#each importantTags() as tag}
				{#if onTagSelect}
					<button class:active-tag={activeTag === tag} class="tag-button" type="button" onclick={() => onTagSelect(tag)}>
						{tag}
					</button>
				{:else}
					<span class="tag-static">{tag}</span>
				{/if}
			{/each}
		</div>
	</div>
</article>

<style>
	.event-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: 0.8rem;
		align-items: start;
		padding: 0.74rem 0.82rem;
		background: #fff;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
	}

	.event-row.critical {
		border-color: rgba(175, 35, 20, 0.38);
		border-left: 3px solid #af2314;
		background: #fff8f5;
	}

	.event-row.critical h3 {
		color: #8f2418;
	}

	h3 a {
		color: inherit;
		text-decoration: none;
	}

	h3 a:hover {
		text-decoration: underline;
		text-underline-offset: 0.18em;
	}

	.event-row.event-high {
		border-color: rgba(255, 174, 76, 0.28);
	}

	.event-content,
	.title-stack {
		display: grid;
		gap: 0.44rem;
		min-width: 0;
	}

	.event-top,
	.label-row,
	.meta-row,
	.detail-strip,
	.footer-row,
	.tx-group {
		display: flex;
		flex-wrap: wrap;
		gap: 0.42rem;
		min-width: 0;
	}

	.event-top {
		align-items: flex-start;
		justify-content: space-between;
		gap: 0.62rem;
	}

	.label-row,
	.footer-row,
	.tx-group {
		align-items: center;
	}

	h3,
	strong,
	span {
		margin: 0;
	}

	h3 {
		font-size: 0.98rem;
		line-height: 1.25;
		letter-spacing: 0;
		overflow-wrap: anywhere;
	}

	.priority-label {
		display: inline-flex;
		align-items: center;
		padding: 0.12rem 0.34rem;
		border-radius: 5px;
		font-size: 0.72rem;
		font-weight: 700;
	}

	.priority-label.priority-high {
		background: rgba(255, 189, 80, 0.18);
		color: #9a5c18;
	}

	.priority-label.priority-critical {
		background: #af2314;
		color: #fff;
	}

	.kind-label {
		display: inline-flex;
		align-items: center;
		font-size: 0.74rem;
		font-weight: 600;
		color: #677482;
	}

	.meta-row {
		color: #6b7a88;
		font-size: 0.74rem;
		justify-content: flex-end;
	}

	.detail-strip {
		gap: 0.36rem 0.52rem;
	}

	.detail-token {
		display: inline-flex;
		align-items: baseline;
		gap: 0.32rem;
		max-width: 100%;
		min-width: 0;
		color: #596775;
		text-decoration: none;
	}

	.detail-token span {
		font-size: 0.7rem;
		font-weight: 600;
		color: #788796;
	}

	.detail-token strong {
		font-size: 0.82rem;
		color: #25313d;
		overflow-wrap: anywhere;
	}

	.fact-token {
		text-decoration: underline;
		text-decoration-color: rgba(23, 32, 42, 0.18);
		text-underline-offset: 0.13em;
	}

	.signature-chip {
		display: inline-flex;
		align-items: center;
		gap: 0.46rem;
		min-width: 0;
		color: #667482;
	}

	.tx-group {
		gap: 0.34rem;
	}

	.signature-chip span {
		font-size: 0.72rem;
		font-weight: 600;
		color: #778592;
	}

	.signature-chip code,
	.detail-token code {
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
		font-size: 0.78rem;
	}

	.icon-button,
	.icon-link {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 1.85rem;
		height: 1.85rem;
		border-radius: 7px;
		border: 1px solid rgba(23, 32, 42, 0.08);
		background: #fff;
		color: #50606f;
		text-decoration: none;
	}

	.icon-button {
		cursor: pointer;
		font-size: 0.8rem;
		font-weight: 800;
	}

	.icon-button svg,
	.icon-link svg {
		width: 1rem;
		height: 1rem;
		fill: currentColor;
	}

	.tag-button,
	.tag-static {
		display: inline-flex;
		align-items: center;
		padding: 0;
		font-size: 0.72rem;
		font-weight: 600;
		background: transparent;
		color: #7a5a44;
		border: none;
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.tag-button {
		cursor: pointer;
	}

	.tag-button.active-tag {
		color: #bd4a17;
	}

	@media (max-width: 920px) {
		.meta-row {
			justify-content: flex-start;
		}
	}
</style>
