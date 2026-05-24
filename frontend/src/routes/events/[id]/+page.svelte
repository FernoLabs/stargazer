<script lang="ts">
	import { explorerAddressUrl, explorerTransactionUrl } from '$lib/explorer';
	import ActivityEventRow from '$lib/components/ActivityEventRow.svelte';
	import {
		eventAlertLevel,
		eventKindLabel,
		eventPrimaryAction,
		eventProtocolHref,
		eventProtocolLabel,
		eventTargetDisplayName,
		eventTitle,
		payloadString
	} from '$lib/event-preview';
	import { formatTimestamp, shortAddress } from '$lib/format';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	type Fact = {
		label: string;
		value: string;
		href?: string;
		external?: boolean;
	};

	function level() {
		return eventAlertLevel(data.event);
	}

	function primaryAction() {
		return eventPrimaryAction(data.event);
	}

	function relatedActivityHref() {
		const search = new URLSearchParams({ target: data.event.target_address });
		if (data.event.target_type === 'program') search.set('scope', 'programs');
		if (data.event.target_type === 'multisig') search.set('scope', 'multisig');
		if (data.event.kind.startsWith('program_idl_')) search.set('scope', 'idl');
		return `/events?${search.toString()}`;
	}

	function targetHref() {
		if (data.event.target_type === 'program') return `/programs/${data.event.target_address}`;
		return explorerAddressUrl(data.event.target_address, data.explorerCluster);
	}

	function targetRel() {
		return data.event.target_type === 'program' ? undefined : 'noreferrer';
	}

	function targetTarget() {
		return data.event.target_type === 'program' ? undefined : '_blank';
	}

	function facts() {
		const actor = data.event.actor;
		const protocol = eventProtocolLabel(data.event, data.defiProtocols);
		const items: Array<Fact | null> = [
			{ label: 'Alert level', value: data.preview.alertLabel },
			{ label: 'Event type', value: eventKindLabel(data.event) },
			protocol ? { label: 'Protocol', value: protocol } : null,
			{ label: 'Target', value: shortAddress(data.event.target_address), href: targetHref() },
			actor
				? {
						label: 'Actor',
						value: payloadString(data.event, 'actor_name') ?? shortAddress(actor),
						href: explorerAddressUrl(actor, data.explorerCluster),
						external: true
					}
				: null,
			{ label: 'Slot', value: new Intl.NumberFormat('en-US').format(data.event.slot) },
			{ label: 'Path', value: data.event.instruction_path },
			{ label: 'Indexed', value: formatTimestamp(data.event.block_time ?? data.event.indexed_at) }
		];

		return items.filter((fact): fact is Fact => Boolean(fact));
	}

	function payloadJson() {
		return JSON.stringify(data.event.payload, null, 2);
	}
</script>

<svelte:head>
	<title>{data.preview.pageTitle}</title>
	<meta name="description" content={data.preview.description} />
	<meta property="og:site_name" content="FireWatch" />
	<meta property="og:type" content="article" />
	<meta property="og:title" content={data.preview.pageTitle} />
	<meta property="og:description" content={data.preview.description} />
	<meta property="og:url" content={data.preview.url} />
	<meta property="og:image" content={data.preview.imageUrl} />
	<meta property="og:image:alt" content={`${data.preview.alertLabel} from FireWatch`} />
	<meta property="og:image:type" content="image/png" />
	<meta property="og:image:width" content="1200" />
	<meta property="og:image:height" content="630" />
	<meta name="twitter:card" content="summary_large_image" />
	<meta name="twitter:title" content={data.preview.pageTitle} />
	<meta name="twitter:description" content={data.preview.description} />
	<meta name="twitter:image" content={data.preview.imageUrl} />
</svelte:head>

<div class="page-shell">
	<section class:critical={level() === 'critical'} class:high={level() === 'high'} class="panel event-header">
		<div class="title-group">
			<div class="label-row">
				<span class="alert-label">{data.preview.alertLabel}</span>
				<span>{eventKindLabel(data.event)}</span>
				{#if eventProtocolLabel(data.event, data.defiProtocols)}
					{#if eventProtocolHref(data.event)}
						<a href={eventProtocolHref(data.event) ?? undefined}>
							{eventProtocolLabel(data.event, data.defiProtocols)}
						</a>
					{:else}
						<span>{eventProtocolLabel(data.event, data.defiProtocols)}</span>
					{/if}
				{/if}
			</div>
			<h1>{eventTitle(data.event, data.defiProtocols)}</h1>
			<p>{data.event.summary}</p>
		</div>

		<div class="action-row">
			{#if primaryAction()}
				<a class="primary-link" href={primaryAction()?.href}>{primaryAction()?.label}</a>
			{/if}
			<a class="secondary-link" href={explorerTransactionUrl(data.event.signature, data.explorerCluster)} rel="noreferrer" target="_blank">
				Open tx
			</a>
			<a class="secondary-link" href={relatedActivityHref()}>Related activity</a>
		</div>
	</section>

	<ActivityEventRow
		event={data.event}
		explorerCluster={data.explorerCluster}
		protocols={data.defiProtocols}
		linkTitle={false}
	/>

	<section class="panel detail-panel">
		<div class="panel-heading">
			<h2>Details</h2>
			<span>{eventTargetDisplayName(data.event, data.defiProtocols)}</span>
		</div>

		<div class="fact-grid">
			{#each facts() as fact}
				<div class="fact-item">
					<span>{fact.label}</span>
					{#if fact.href}
						<a href={fact.href} rel={fact.external ? 'noreferrer' : targetRel()} target={fact.external ? '_blank' : targetTarget()}>{fact.value}</a>
					{:else}
						<strong>{fact.value}</strong>
					{/if}
				</div>
			{/each}
		</div>

		<div class="code-blocks">
			<div>
				<span>Event id</span>
				<code>{data.event.id}</code>
			</div>
			<div>
				<span>Signature</span>
				<code>{data.event.signature}</code>
			</div>
		</div>
	</section>

	<section class="panel payload-panel">
		<div class="panel-heading">
			<h2>Payload</h2>
			<span>{data.event.tags.length} tags</span>
		</div>

		<div class="tag-list">
			{#each data.event.tags as tag}
				<a href={`/events?tag=${encodeURIComponent(tag)}`}>{tag}</a>
			{/each}
		</div>

		<pre>{payloadJson()}</pre>
	</section>
</div>

<style>
	.page-shell {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem 0 2.4rem;
		display: grid;
		gap: 0.86rem;
	}

	.panel {
		background: #fff;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 10px;
		padding: 0.9rem 1rem;
	}

	.event-header {
		display: flex;
		justify-content: space-between;
		gap: 1rem;
		align-items: flex-start;
	}

	.event-header.critical {
		border-color: rgba(175, 35, 20, 0.38);
		border-left: 3px solid #af2314;
		background: #fff8f5;
	}

	.event-header.high {
		border-color: rgba(183, 112, 35, 0.3);
		border-left: 3px solid #b77023;
	}

	.title-group {
		display: grid;
		gap: 0.45rem;
		min-width: 0;
	}

	.label-row,
	.action-row,
	.panel-heading,
	.tag-list {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem;
	}

	.label-row {
		color: #5f6c79;
		font-size: 0.86rem;
	}

	.alert-label {
		color: #8f2418;
		font-weight: 720;
	}

	h1,
	h2,
	p {
		margin: 0;
	}

	h1 {
		font-size: clamp(1.35rem, 2vw, 2rem);
		line-height: 1.08;
		letter-spacing: 0;
	}

	h2 {
		font-size: 1rem;
	}

	p {
		color: #48525c;
		max-width: 72ch;
		line-height: 1.5;
	}

	a {
		color: #17202a;
	}

	.action-row {
		justify-content: flex-end;
	}

	.primary-link,
	.secondary-link,
	.tag-list a {
		display: inline-flex;
		align-items: center;
		min-height: 2.1rem;
		padding: 0 0.7rem;
		border-radius: 8px;
		text-decoration: none;
		font-weight: 700;
		font-size: 0.9rem;
	}

	.primary-link {
		background: #17202a;
		color: #fff;
	}

	.secondary-link,
	.tag-list a {
		border: 1px solid rgba(23, 32, 42, 0.12);
		background: #fff;
	}

	.detail-panel,
	.payload-panel {
		display: grid;
		gap: 0.8rem;
	}

	.panel-heading {
		justify-content: space-between;
		color: #5f6c79;
	}

	.panel-heading h2 {
		color: #17202a;
	}

	.fact-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
		gap: 0.6rem;
	}

	.fact-item {
		display: grid;
		gap: 0.18rem;
		min-width: 0;
		padding: 0.64rem 0;
		border-top: 1px solid rgba(23, 32, 42, 0.08);
	}

	.fact-item span,
	.code-blocks span {
		color: #6b7680;
		font-size: 0.82rem;
	}

	.fact-item strong,
	.fact-item a,
	code {
		min-width: 0;
		overflow-wrap: anywhere;
	}

	.code-blocks {
		display: grid;
		gap: 0.55rem;
	}

	.code-blocks div {
		display: grid;
		gap: 0.22rem;
	}

	code,
	pre {
		font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
	}

	code {
		font-size: 0.82rem;
		color: #17202a;
	}

	pre {
		margin: 0;
		padding: 0.8rem;
		overflow: auto;
		border-radius: 8px;
		background: #101418;
		color: #edf1f5;
		font-size: 0.82rem;
		line-height: 1.45;
	}

	@media (max-width: 720px) {
		.event-header {
			display: grid;
		}

		.action-row {
			justify-content: flex-start;
		}
	}
</style>
