<script lang="ts">
	import { explorerAddressUrl, explorerIdlUrl } from '$lib/explorer';
	import {
		joinCommandLines,
		watchMultisigCommand,
		watchProgramCommand,
		watchProtocolCommand
	} from '$lib/bot-commands';
	import { programPageHref, protocolPageHref, syntheticSquadsDescriptor } from '$lib/catalog';
	import ActivityEventRow from '$lib/components/ActivityEventRow.svelte';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import { shortAddress } from '$lib/format';
	import { programCriticalHref, programRiskSummary, riskGradeTone } from '$lib/risk';
	import type { ChainEvent, DefiProtocolDescriptor, EventsResponse } from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	let appendedEvents = $state<ChainEvent[]>([]);
	let nextOffsetOverride = $state<number | null | undefined>(undefined);
	let loadingMore = $state(false);
	let loadMoreError = $state<string | null>(null);
	let copiedState = $state<'idle' | 'copied' | 'error'>('idle');

	type WatchTarget = {
		key: string;
		label: string;
		address: string | null;
		command: string;
		kind: 'project' | 'program' | 'multisig';
		reason: string;
		context: string;
	};

	function protocol() {
		return data.protocol as DefiProtocolDescriptor | null;
	}

	function catalogProtocols() {
		const current = protocol();
		if (!current) return [syntheticSquadsDescriptor()];
		if (current.slug === 'squads') return [current];
		return [current, syntheticSquadsDescriptor()];
	}

	function allEvents() {
		if (appendedEvents.length === 0) return data.events;
		const seen = new Set(data.events.map((event) => event.id));
		return [...data.events, ...appendedEvents.filter((event) => !seen.has(event.id))];
	}

	function currentNextOffset() {
		return nextOffsetOverride === undefined ? data.nextOffset : nextOffsetOverride;
	}

	function payloadString(event: ChainEvent, key: string) {
		const value = event.payload[key];
		return typeof value === 'string' && value.length > 0 ? value : null;
	}

	function protocolMultisigs() {
		const addresses = new Set<string>();
		for (const event of allEvents()) {
			if (event.target_type === 'multisig') {
				addresses.add(event.target_address);
			}
			const payloadMultisig = payloadString(event, 'multisig');
			if (payloadMultisig) {
				addresses.add(payloadMultisig);
			}
		}
		return [...addresses];
	}

	function coverageReason() {
		const coverage = protocol()?.coverage;
		if (coverage === 'deep_decode') {
			return 'Upgrades, IDL changes, admin instructions, and unknown review on this program.';
		}
		if (coverage === 'admin_signals') {
			return 'Upgrades, IDL changes, and high-impact admin signals on this program.';
		}
		return 'Upgrades and FireWatch-known address activity on this program.';
	}

	function multisigEvidence(events: ChainEvent[]) {
		const hasExecution = events.some((event) =>
			['multisig_vault_executed', 'multisig_config_executed'].includes(event.kind)
		);
		const hasApproval = events.some((event) => event.kind === 'multisig_proposal_approved');
		const hasActivation = events.some((event) => event.kind === 'multisig_proposal_activated');
		const hasRejection = events.some((event) =>
			['multisig_proposal_rejected', 'multisig_proposal_cancelled'].includes(event.kind)
		);

		if (hasExecution && hasApproval) return 'Seen approving and executing protocol changes.';
		if (hasExecution) return 'Seen executing protocol changes.';
		if (hasApproval && hasActivation) return 'Seen creating and approving proposals.';
		if (hasApproval) return 'Seen approving protocol proposals.';
		if (hasActivation) return 'Seen creating protocol proposals.';
		if (hasRejection) return 'Seen handling proposal outcomes.';
		return 'Seen in governance activity linked to this protocol.';
	}

	function watchTargets(): WatchTarget[] {
		const currentProtocol = protocol();
		if (!currentProtocol) return [];

		const projectTargets = [
			{
				key: `project:${currentProtocol.slug}`,
				label: currentProtocol.name,
				address: currentProtocol.slug,
				command: watchProtocolCommand(currentProtocol.slug),
				kind: 'project' as const,
				reason: projectWatchReason(currentProtocol),
				context: 'Protocol rule'
			}
		];

		const programTargets = currentProtocol.programs.map((program) => ({
			key: `program:${program.program_id}`,
			label: program.label,
			address: program.program_id,
			command: watchProgramCommand(program.program_id),
			kind: 'program' as const,
			reason: coverageReason(),
			context: 'Program'
		}));

		const groupedMultisigs = new Map<string, ChainEvent[]>();
		for (const event of allEvents()) {
			const addresses = new Set<string>();
			if (event.target_type === 'multisig') {
				addresses.add(event.target_address);
			}
			const payloadMultisig = payloadString(event, 'multisig');
			if (payloadMultisig) {
				addresses.add(payloadMultisig);
			}

			for (const address of addresses) {
				const events = groupedMultisigs.get(address) ?? [];
				events.push(event);
				groupedMultisigs.set(address, events);
			}
		}

		const multisigTargets = [...groupedMultisigs.entries()]
			.sort((left, right) => right[1].length - left[1].length)
			.map(([address, events]) => ({
				key: `multisig:${address}`,
				label: shortAddress(address),
				address,
				command: watchMultisigCommand(address),
				kind: 'multisig' as const,
				reason: multisigEvidence(events),
				context: `${events.length} governance event${events.length === 1 ? '' : 's'}`
			}));

		return [...projectTargets, ...programTargets, ...multisigTargets];
	}

	function projectWatchReason(currentProtocol: DefiProtocolDescriptor) {
		const parts = [];
		if (currentProtocol.programs.length > 0) {
			parts.push(`${currentProtocol.programs.length} program${currentProtocol.programs.length === 1 ? '' : 's'}`);
		}
		if (currentProtocol.sol_domains.length > 0) {
			parts.push(`${currentProtocol.sol_domains.length} SNS domain${currentProtocol.sol_domains.length === 1 ? '' : 's'}`);
		}
		if (currentProtocol.amount_triggers.length > 0) {
			parts.push(`${currentProtocol.amount_triggers.length} amount trigger${currentProtocol.amount_triggers.length === 1 ? '' : 's'}`);
		}
		if (parts.length === 0) return 'Tracks this protocol under one protocol rule.';
		return `Tracks ${parts.join(' and ')} under one protocol rule.`;
	}

	function commandBlock() {
		return joinCommandLines(watchTargets().map((item) => item.command));
	}

	function stats() {
		const current = protocol();
		return [
			{ label: 'Programs', value: String(current?.programs.length ?? 0) },
			{ label: 'SNS', value: String(current?.sol_domains.length ?? 0) },
			{ label: 'Triggers', value: String(current?.amount_triggers.length ?? 0) },
			{ label: 'Multisigs', value: String(protocolMultisigs().length) },
			{ label: 'Loaded', value: String(allEvents().length) }
		];
	}

	async function copyText(text: string) {
		try {
			await navigator.clipboard.writeText(text);
			copiedState = 'copied';
		} catch {
			copiedState = 'error';
		}
	}

	async function loadMore() {
		const offset = currentNextOffset();
		if (loadingMore || offset === null) return;

		loadingMore = true;
		loadMoreError = null;

		try {
			const search = new URLSearchParams({ limit: '120', offset: String(offset) });
			if (protocol()) {
				search.set('protocol', protocol()!.slug);
				search.set('include_related_multisigs', 'true');
			}

			const response = await fetch(`${data.backendBaseUrl}/api/events?${search.toString()}`);
			if (!response.ok) throw new Error(`Backend returned ${response.status}`);

			const payload = (await response.json()) as EventsResponse;
			const seenIds = new Set(allEvents().map((event) => event.id));
			appendedEvents = [...appendedEvents, ...payload.items.filter((event) => !seenIds.has(event.id))];
			nextOffsetOverride = payload.next_offset;
		} catch (error) {
			loadMoreError = error instanceof Error ? error.message : 'Unknown backend error';
		} finally {
			loadingMore = false;
		}
	}
</script>

<svelte:head>
	<title>{`${protocol()?.name ?? 'Protocol'} · FireWatch`}</title>
</svelte:head>

<div class="page-shell">
	<section class="panel page-top">
			<div class="title-row">
				<ProtocolIdentity slug={protocol()?.slug ?? null} label={protocol()?.name ?? 'Protocol'} />
				<div class="title-copy">
					<h1>{protocol()?.name}</h1>
					<p class="lede">Tracked SNS domains, programs, trigger rules, linked multisigs, and recent activity in one place.</p>
				</div>
		</div>

		<div class="page-actions">
			<a class="primary-link" href={`/events?protocol=${protocol()?.slug}`}>Protocol activity</a>
			<a class="secondary-link" href="/docs">Bot reference</a>
		</div>

			<div class="stats-strip">
				{#each stats() as stat}
					<span class="stat-item">
						<strong>{stat.value}</strong>
						{stat.label}
					</span>
				{/each}
			</div>

		{#if data.protocolEventsError}
			<p class="flash error">Activity unavailable: {data.protocolEventsError}</p>
		{/if}
	</section>

	<section class="overview-grid">
		<article class="panel">
			<SectionHeader
				title="Watch targets"
				description="Copy one focused rule for the project, or track individual programs and multisigs."
				compact
			/>

			<div class="command-list">
				{#each watchTargets() as item}
					<div class="command-row">
						<div class="command-copy">
							<div class="command-head">
								<span class={`command-kind ${item.kind}`}>{item.context}</span>
								<strong>{item.label}</strong>
								{#if item.address}
									<code>{item.kind === 'project' ? item.address : shortAddress(item.address)}</code>
								{/if}
							</div>
							<p>{item.reason}</p>
							<code class="command-code">{item.command}</code>
						</div>
						<button class="ghost-button" type="button" onclick={() => copyText(item.command)}>Copy</button>
					</div>
				{/each}
			</div>

			<div class="panel-actions">
				<button class="secondary-link button-like" type="button" onclick={() => copyText(commandBlock())}>
					Copy all commands
				</button>
			</div>

			{#if copiedState === 'copied'}
				<p class="flash success">Command copied.</p>
			{:else if copiedState === 'error'}
				<p class="flash error">Clipboard access failed. Copy the command manually.</p>
			{/if}
		</article>

		<article class="panel">
			<SectionHeader
				title="Monitors"
				description="Programs, SNS domains, and proposed amount triggers attached to this project."
				compact
			/>

			<div class="surface-list">
				{#each protocol()?.programs ?? [] as program}
					{@const risk = programRiskSummary(program)}
					<div class="surface-row">
						<div class="surface-copy">
							<a class="surface-name" href={programPageHref(program.program_id)}>{program.label}</a>
							<code>{shortAddress(program.program_id)}</code>
							<a class="risk-strip" href={programCriticalHref(program.program_id)}>
								<span class={`risk-grade grade-${riskGradeTone(risk)}`}>Rug {risk.grade}</span>
								<span>Score <strong>{risk.score}</strong></span>
								<span>Critical <strong>{risk.critical_total}</strong></span>
								<span>Unreviewed <strong>{risk.unreviewed_critical}</strong></span>
								<span>Reviewed <strong>{risk.reviewed_critical}</strong></span>
							</a>
						</div>
						<div class="surface-links">
							<a href={explorerAddressUrl(program.program_id, data.explorerCluster)} rel="noreferrer" target="_blank">
								Explorer
							</a>
							{#if program.idl_metadata_address}
								<a href={explorerIdlUrl(program.program_id, data.explorerCluster)} rel="noreferrer" target="_blank">
									IDL
								</a>
							{/if}
							<a href={programPageHref(program.program_id)}>Open</a>
						</div>
					</div>
				{/each}

				{#each protocol()?.sol_domains ?? [] as domain}
					<div class="surface-row">
						<div class="surface-copy">
							<span class="surface-name">{domain}</span>
							<code>SNS root domain</code>
						</div>
						<div class="surface-links">
							<a href={`/events?protocol=${protocol()?.slug}`}>Activity</a>
						</div>
					</div>
				{/each}

				{#each protocol()?.amount_triggers ?? [] as trigger}
					<div class="surface-row">
						<div class="surface-copy">
							<span class="surface-name">{trigger.label}</span>
							<p>{trigger.description}</p>
						</div>
						<div class="surface-links">
							<span class="surface-tag">Amount trigger</span>
						</div>
					</div>
				{/each}
			</div>

			{#if (protocol()?.programs.length ?? 0) === 0 && (protocol()?.sol_domains.length ?? 0) === 0 && (protocol()?.amount_triggers.length ?? 0) === 0}
				<p class="inline-note">No monitors have been attached to this project yet.</p>
			{:else if protocolMultisigs().length === 0}
				<p class="inline-note">No related multisig has been identified in indexed history for this protocol yet.</p>
			{:else}
				<p class="inline-note">Related multisigs are listed in the watchable pubkeys panel with the reason FireWatch linked them to this protocol.</p>
			{/if}
		</article>
	</section>

	<section class="panel">
		<SectionHeader
			title="Recent activity"
			description={`${allEvents().length} recent domain, program, and multisig events in this protocol view.`}
			href={`/events?protocol=${protocol()?.slug}`}
			hrefLabel="Open full activity"
			compact
		/>

		{#if allEvents().length === 0}
			<p class="empty-state">No recent events for this view yet.</p>
		{:else}
			<div class="event-list">
				{#each allEvents() as event}
					<ActivityEventRow
						{event}
						explorerCluster={data.explorerCluster}
						protocols={catalogProtocols()}
						fallbackProtocolSlug={protocol()?.slug ?? null}
						fallbackProtocolLabel={protocol()?.name ?? null}
					/>
				{/each}
			</div>
		{/if}
	</section>

	{#if currentNextOffset() !== null}
		<div class="load-row">
			<button class="primary-link button-like" type="button" onclick={loadMore} disabled={loadingMore}>
				{loadingMore ? 'Loading…' : 'Load more'}
			</button>
			{#if loadMoreError}
				<p class="flash error">{loadMoreError}</p>
			{/if}
		</div>
	{/if}
</div>

<style>
	.page-shell {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem 0 2.4rem;
		display: grid;
		gap: 0.76rem;
	}

	.panel {
		background: #fff;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 10px;
		padding: 0.96rem 1rem;
	}

	.page-top,
	.title-row,
	.title-copy,
	.command-list,
	.surface-list,
	.event-list {
		display: grid;
		gap: 0.68rem;
	}

	.title-row {
		grid-template-columns: auto minmax(0, 1fr);
		align-items: start;
		min-width: 0;
	}

	h1,
	strong,
	p {
		margin: 0;
	}

	h1 {
		font-size: 1.85rem;
		line-height: 1.02;
		letter-spacing: 0;
		overflow-wrap: anywhere;
	}

	.lede,
	.command-copy p,
	.inline-note,
	.empty-state,
	.flash {
		color: #5f6c79;
	}

	.lede {
		max-width: 60ch;
		line-height: 1.5;
	}

	.page-actions,
	.stats-strip,
	.surface-links,
	.load-row,
	.panel-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.52rem;
	}

	.stats-strip {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
		gap: 1px;
		overflow: hidden;
		padding: 0;
		border-radius: 8px;
		background: var(--fw-border);
	}

	.stat-item {
		display: inline-flex;
		flex-direction: column;
		gap: 0.06rem;
		min-width: 0;
		padding: 0.62rem 0.68rem;
		background: var(--fw-panel-soft);
		font-size: 0.76rem;
		font-weight: 600;
		color: #62707d;
	}

	.stat-item strong {
		color: #17202a;
		font-size: 0.9rem;
		overflow-wrap: anywhere;
	}

	.overview-grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 0.76rem;
	}

	.command-row,
	.surface-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: start;
		gap: 0.72rem;
		padding: 0.72rem 0.78rem;
		border-radius: 8px;
		background: #fbfaf8;
		border: 1px solid rgba(23, 32, 42, 0.08);
	}

	.command-copy,
	.command-head,
	.surface-copy {
		display: grid;
		gap: 0.18rem;
	}

	.command-copy {
		min-width: 0;
	}

	.surface-copy {
		min-width: 0;
	}

	.command-head {
		gap: 0.14rem;
	}

	.command-kind {
		display: inline-flex;
		width: fit-content;
		font-size: 0.72rem;
		font-weight: 600;
	}

	.command-kind.program {
		color: #bd4a17;
	}

	.command-kind.project {
		color: #7b5b45;
	}

	.command-kind.multisig {
		color: #50606f;
	}

	code {
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
		font-size: 0.8rem;
		color: #63707c;
		overflow-wrap: anywhere;
	}

	.command-code {
		color: #1c2832;
	}

	.surface-copy code {
		font-size: 0.76rem;
	}

	.surface-copy p {
		margin: 0;
		color: #5f6c79;
		font-size: 0.84rem;
		line-height: 1.45;
	}

	.risk-strip {
		display: flex;
		flex-wrap: wrap;
		gap: 0.34rem 0.48rem;
		padding-top: 0.22rem;
		color: #596775;
		text-decoration: none;
	}

	.risk-strip span {
		font-size: 0.74rem;
		font-weight: 700;
	}

	.risk-strip strong {
		color: #17202a;
		font-size: 0.78rem;
	}

	.risk-grade {
		color: #216d4d;
	}

	.risk-grade.grade-c,
	.risk-grade.grade-d {
		color: #8a5e1e;
	}

	.risk-grade.grade-f {
		color: #9c3929;
	}

	.surface-name,
	.surface-links a,
	.text-link {
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.surface-name {
		font-weight: 800;
		color: #1f2933;
	}

	.surface-links a {
		font-size: 0.82rem;
		font-weight: 800;
		color: #556473;
	}

	.surface-tag {
		display: inline-flex;
		align-items: center;
		min-height: 1.42rem;
		padding: 0.16rem 0.42rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 6px;
		background: #fbfaf8;
		color: #596775;
		font-size: 0.74rem;
		font-weight: 700;
	}

	.primary-link,
	.secondary-link,
	.ghost-button {
		display: inline-flex;
		flex: 0 0 auto;
		align-items: center;
		justify-content: center;
		padding: 0.66rem 0.86rem;
		border-radius: 8px;
		font-size: 0.86rem;
		font-weight: 600;
		text-decoration: none;
	}

	.primary-link {
		border: 1px solid #bd4a17;
		background: #bd4a17;
		color: white;
	}

	.secondary-link,
	.ghost-button {
		background: #fff;
		color: #556473;
		border: 1px solid rgba(23, 32, 42, 0.12);
		cursor: pointer;
	}

	.button-like {
		cursor: pointer;
	}

	.inline-note {
		padding: 0.74rem 0.8rem;
		border-radius: 8px;
		background: #fbfaf8;
		border: 1px solid rgba(23, 32, 42, 0.08);
	}

	.flash {
		padding: 0.74rem 0.82rem;
		border-radius: 8px;
		font-size: 0.88rem;
		font-weight: 600;
	}

	.flash.success {
		background: rgba(61, 140, 99, 0.14);
		color: #216d4d;
	}

	.flash.error {
		background: rgba(191, 74, 57, 0.14);
		color: #9c3929;
	}

	.load-row {
		align-items: center;
	}

	@media (max-width: 920px) {
		.overview-grid {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 720px) {
		.page-shell {
			width: min(100vw - 1rem, var(--fw-page-max));
			padding-top: 0.8rem;
		}

		.title-row {
			grid-template-columns: 1fr;
		}

		.command-row,
		.surface-row {
			grid-template-columns: 1fr;
		}

		.stat-item:last-child:nth-child(odd) {
			grid-column: 1 / -1;
		}
	}
</style>
