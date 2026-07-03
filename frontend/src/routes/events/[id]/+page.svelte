<script lang="ts">
	import { resolve } from '$app/paths';
	import { explorerAddressUrl, explorerTransactionUrl } from '$lib/explorer';
	import ActivityEventRow from '$lib/components/ActivityEventRow.svelte';
	import {
		eventAlertLevel,
		eventKindLabel,
		eventProtocolLabel,
		eventProtocolSlug,
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
		programAddress?: string;
	};

	function level() {
		return eventAlertLevel(data.event);
	}

	function relatedActivityHref() {
		const search = new URLSearchParams({ target: data.event.target_address });
		if (data.event.target_type === 'program') search.set('scope', 'programs');
		if (data.event.target_type === 'multisig') search.set('scope', 'multisig');
		if (data.event.kind.startsWith('program_idl_')) search.set('scope', 'idl');
		return search.toString();
	}

	function goBack(event: MouseEvent) {
		if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || event.button !== 0) return;
		if (!document.referrer || window.history.length <= 1) return;

		const referrer = new URL(document.referrer);
		if (referrer.origin !== window.location.origin) return;
		if (referrer.pathname !== resolve('/events')) return;

		event.preventDefault();
		window.history.back();
	}

	function openExternal(url: string) {
		window.open(url, '_blank', 'noopener,noreferrer');
	}

	function primaryProgramAddress() {
		return payloadString(data.event, 'program') ?? data.event.target_address;
	}

	function primaryVersionId() {
		return payloadString(data.event, 'program_version_id');
	}

	function facts() {
		const actor = data.event.actor;
		const protocol = eventProtocolLabel(data.event, data.defiProtocols);
		const items: Array<Fact | null> = [
			{ label: 'Alert level', value: data.preview.alertLabel },
			{ label: 'Event type', value: eventKindLabel(data.event) },
			protocol ? { label: 'Protocol', value: protocol } : null,
			data.event.target_type === 'program'
				? {
						label: 'Target',
						value: shortAddress(data.event.target_address),
						programAddress: data.event.target_address
					}
				: {
						label: 'Target',
						value: shortAddress(data.event.target_address),
						href: explorerAddressUrl(data.event.target_address, data.explorerCluster),
						external: true
					},
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
	<nav class="back-nav" aria-label="Event navigation">
		<a href={resolve('/events')} onclick={goBack}>
			<svg viewBox="0 0 20 20" aria-hidden="true">
				<path d="M8.53 4.47a.75.75 0 0 1 0 1.06L4.81 9.25H17a.75.75 0 0 1 0 1.5H4.81l3.72 3.72a.75.75 0 1 1-1.06 1.06l-5-5a.75.75 0 0 1 0-1.06l5-5a.75.75 0 0 1 1.06 0z"></path>
			</svg>
			<span>Back to feed</span>
		</a>
	</nav>

	<section class:critical={level() === 'critical'} class:high={level() === 'high'} class="panel event-header">
		<div class="title-group">
			<div class="label-row">
				<span class="alert-label">{data.preview.alertLabel}</span>
				<span>{eventKindLabel(data.event)}</span>
				{#if eventProtocolLabel(data.event, data.defiProtocols)}
					{#if eventProtocolSlug(data.event)}
						<a href={resolve('/protocols/[slug]', { slug: eventProtocolSlug(data.event) ?? '' })}>
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
			{#if data.event.target_type === 'program'}
				{#if primaryVersionId()}
					<a
						class="primary-link"
						href={resolve('/programs/[address]/versions/[version_id]/compare', {
							address: primaryProgramAddress(),
							version_id: primaryVersionId() ?? ''
						})}
					>
						Code diff
					</a>
				{:else}
					<a class="primary-link" href={resolve('/programs/[address]', { address: primaryProgramAddress() })}>
						Program page
					</a>
				{/if}
			{/if}
			<button class="secondary-link" type="button" onclick={() => openExternal(explorerTransactionUrl(data.event.signature, data.explorerCluster))}>
				Open tx
			</button>
			<a class="secondary-link" href={resolve(`/events?${relatedActivityHref()}`)}>Related activity</a>
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
			{#each facts() as fact (fact.label)}
				<div class="fact-item">
					<span>{fact.label}</span>
					{#if fact.programAddress}
						<a href={resolve('/programs/[address]', { address: fact.programAddress })}>{fact.value}</a>
					{:else if fact.href}
						{#if fact.external}
							<button class="fact-link" type="button" onclick={() => openExternal(fact.href ?? '')}>{fact.value}</button>
						{:else}
							<strong>{fact.value}</strong>
						{/if}
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
			{#each data.event.tags as tag (tag)}
				<a href={resolve(`/events?tag=${encodeURIComponent(tag)}`)}>{tag}</a>
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

	.back-nav {
		display: flex;
		align-items: center;
		min-width: 0;
	}

	.back-nav a {
		display: inline-flex;
		align-items: center;
		gap: 0.34rem;
		width: max-content;
		max-width: 100%;
		min-height: 1.9rem;
		color: #4b5865;
		font-size: 0.86rem;
		font-weight: 720;
		text-decoration: none;
	}

	.back-nav a:hover {
		color: #17202a;
		text-decoration: underline;
		text-underline-offset: 0.14em;
	}

	.back-nav svg {
		width: 0.92rem;
		height: 0.92rem;
		flex: 0 0 auto;
		fill: currentColor;
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

	button.secondary-link,
	.fact-link {
		font: inherit;
		cursor: pointer;
	}

	.fact-link {
		width: max-content;
		max-width: 100%;
		padding: 0;
		border: 0;
		background: transparent;
		color: #17202a;
		text-align: left;
		text-decoration: underline;
		text-underline-offset: 0.12em;
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
	.fact-link,
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
		.page-shell {
			width: min(100vw - 1.25rem, var(--fw-page-max));
			padding-top: 0.62rem;
			gap: 0.72rem;
		}

		.panel {
			border-radius: 8px;
			padding: 0.82rem 0.78rem;
		}

		.event-header {
			display: grid;
		}

		.action-row {
			justify-content: flex-start;
		}
	}

	@media (max-width: 640px) {
		.event-header {
			gap: 0.78rem;
		}

		.label-row,
		.panel-heading,
		.tag-list {
			gap: 0.36rem 0.46rem;
		}

		h1 {
			font-size: 1.28rem;
			line-height: 1.14;
		}

		p {
			font-size: 0.92rem;
		}

		.action-row {
			display: grid;
			grid-template-columns: 1fr;
			width: 100%;
			gap: 0.46rem;
		}

		.primary-link,
		.secondary-link,
		.tag-list a {
			justify-content: center;
			min-width: 0;
		}

		.fact-grid {
			grid-template-columns: 1fr;
			gap: 0.2rem;
		}

		.panel-heading {
			align-items: flex-start;
		}

		.tag-list a {
			max-width: 100%;
			min-height: 1.92rem;
			padding: 0 0.54rem;
			font-size: 0.78rem;
			overflow-wrap: anywhere;
		}

		pre {
			padding: 0.7rem;
			font-size: 0.76rem;
		}
	}
</style>
