<script lang="ts">
	import { nextPageOffset } from '$lib/api-pagination';
	import { explorerAddressUrl, explorerIdlUrl, explorerTransactionUrl } from '$lib/explorer';
	import { protocolPageHref } from '$lib/catalog';
	import ActivityEventRow from '$lib/components/ActivityEventRow.svelte';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import { formatMetricValue, formatTimestamp, readableInstructionName, shortAddress } from '$lib/format';
	import { compactLongTokens, eventDetailHref } from '$lib/event-preview';
	import type {
		ChainEvent,
		EventsResponse,
		ProgramIdlVersionRecord,
		ProgramVersionRecord
	} from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	let appendedEvents = $state<ChainEvent[]>([]);
	let nextOffsetOverride = $state<number | null | undefined>(undefined);
	let loadingMore = $state(false);
	let loadMoreError = $state<string | null>(null);
	let searchQuery = $state('');
	let activityFilter = $state<'all' | 'critical' | 'high' | 'standard'>('all');

	type InstructionFlagRow = {
		name: string;
		admin: boolean;
		highImpact: boolean;
		loadedEvents: number;
		metricEvents: number;
	};

	function allEvents() {
		if (appendedEvents.length === 0) return data.events;
		const seen = new Set(data.events.map((event) => event.id));
		return [...data.events, ...appendedEvents.filter((event) => !seen.has(event.id))];
	}

	function currentNextOffset() {
		return nextOffsetOverride === undefined ? data.nextOffset : nextOffsetOverride;
	}

	function protocolHref() {
		return data.catalogedProgram ? protocolPageHref(data.protocol.slug) : null;
	}

	function programLede() {
		if (data.catalogedProgram) {
			return 'Program-specific loader history, IDL state, flagged instructions, value rules, and signer activity.';
		}
		return 'A watched program address with loader-uploaded versions, raw byte downloads, and indexed activity.';
	}

	function formatByteCount(value: number | null) {
		if (value === null) return 'Unknown';
		return new Intl.NumberFormat('en-US').format(value);
	}

	function shortDigest(value: string | null) {
		if (!value) return 'Pending';
		if (value.length <= 18) return value;
		return `${value.slice(0, 8)}...${value.slice(-8)}`;
	}

	function originLabel(origin: string) {
		if (origin === 'program_metadata') return 'Program Metadata';
		if (origin === 'anchor_legacy') return 'Anchor legacy';
		return origin.replaceAll('_', ' ');
	}

	function shortSignature(signature: string) {
		if (signature.length <= 16) return signature;
		return `${signature.slice(0, 6)}...${signature.slice(-6)}`;
	}

	function versionLabel(version: ProgramVersionRecord) {
		return version.kind === 'program_deployed' ? 'Deploy' : 'Upgrade';
	}

	function canCompareVersion(version: ProgramVersionRecord) {
		return Boolean(version.previous_version_id && version.complete);
	}

	function diffableVersionCount() {
		return data.versions.filter(canCompareVersion).length;
	}

	function formatCount(value: number | null | undefined) {
		return new Intl.NumberFormat('en-US').format(value ?? 0);
	}

	function latestVersion() {
		if (data.versions.length === 0) return null;
		return data.versions.reduce((latest, version) => (version.slot > latest.slot ? version : latest));
	}

	function latestIdlVersion() {
		if (data.idlVersions.length === 0) return null;
		return data.idlVersions.reduce((latest, version) => (version.slot > latest.slot ? version : latest));
	}

	function latestUpdateSlotLabel() {
		const version = latestVersion();
		return version ? formatCount(version.slot) : 'None';
	}

	function latestUpdateDescription() {
		const version = latestVersion();
		if (!version) return 'No loader upload has been extracted for this address yet.';
		return `${versionLabel(version)} at ${formatTimestamp(version.block_time)}.`;
	}

	function idlAccountLinks() {
		const links: Array<{ label: string; address: string; href: string }> = [];
		if (data.program.idl_metadata_address) {
			links.push({
				label: 'Program Metadata IDL',
				address: data.program.idl_metadata_address,
				href: explorerIdlUrl(data.program.program_id, data.explorerCluster)
			});
		}
		if (data.program.anchor_legacy_idl_address) {
			links.push({
				label: 'Anchor legacy IDL',
				address: data.program.anchor_legacy_idl_address,
				href: explorerAddressUrl(data.program.anchor_legacy_idl_address, data.explorerCluster)
			});
		}
		return links;
	}

	function idlStateDescription() {
		if (data.idlVersions.length > 0) {
			return `${data.idlVersions.length} extracted IDL version${data.idlVersions.length === 1 ? '' : 's'} available.`;
		}
		if (idlAccountLinks().length > 0) {
			return 'IDL accounts are known, but no extracted IDL payload is available yet.';
		}
		if (data.protocol.catalog_instruction_count > 0) {
			return 'No IDL account is known; FireWatch uses its cataloged instruction map as a partial view.';
		}
		return 'No IDL account or cataloged instruction map is available for this program yet.';
	}

	function eventPayloadString(event: ChainEvent, key: string) {
		const value = event.payload[key];
		return typeof value === 'string' && value.length > 0 ? value : null;
	}

	function eventPayloadNumber(event: ChainEvent, key: string) {
		const value = event.payload[key];
		return typeof value === 'number' ? value : null;
	}

	function instructionEvents(instruction: string) {
		return allEvents().filter((event) => eventPayloadString(event, 'instruction_name') === instruction);
	}

	function normalizedSearch() {
		return searchQuery.trim().toLowerCase();
	}

	function textMatchesSearch(...values: Array<string | number | null | undefined>) {
		const query = normalizedSearch();
		if (!query) return true;
		return values
			.filter((value) => value !== null && value !== undefined)
			.map(String)
			.some((value) => value.toLowerCase().includes(query));
	}

	function eventMatchesFilter(event: ChainEvent) {
		if (activityFilter === 'all') return true;
		return event.tags.includes(`severity:${activityFilter}`);
	}

	function eventMatchesSearch(event: ChainEvent) {
		return textMatchesSearch(
			event.summary,
			event.kind,
			event.signature,
			event.target_address,
			event.actor,
			event.tags.join(' '),
			eventPayloadString(event, 'instruction_name')
		);
	}

	function filteredEvents() {
		return allEvents().filter((event) => eventMatchesFilter(event) && eventMatchesSearch(event));
	}

	function metricAlertEvents() {
		return allEvents().filter(
			(event) => event.kind === 'program_metric_alert' || event.tags.includes('signal:metric_anomaly')
		);
	}

	function unknownInstructionEvents() {
		return allEvents().filter((event) => event.tags.includes('signal:unknown_instruction'));
	}

	function instructionFlagRows(): InstructionFlagRow[] {
		const names = new Set([
			...data.protocol.admin_instruction_filters,
			...data.protocol.high_impact_instruction_filters
		]);

		return [...names]
			.map((name) => {
				const events = instructionEvents(name);
				return {
					name,
					admin: data.protocol.admin_instruction_filters.includes(name),
					highImpact: data.protocol.high_impact_instruction_filters.includes(name),
					loadedEvents: events.length,
					metricEvents: events.filter(
						(event) =>
							event.kind === 'program_metric_alert' || event.tags.includes('signal:metric_anomaly')
					).length
				};
			})
			.sort((left, right) => {
				if (left.highImpact !== right.highImpact) return left.highImpact ? -1 : 1;
				if (left.admin !== right.admin) return left.admin ? -1 : 1;
				return left.name.localeCompare(right.name);
			});
	}

	function filteredInstructionFlagRows() {
		return instructionFlagRows().filter((row) =>
			textMatchesSearch(
				row.name,
				readableInstructionName(row.name),
				row.admin ? 'admin' : '',
				row.highImpact ? 'critical high impact' : ''
			)
		);
	}

	function showUnknownInstructionRow() {
		return textMatchesSearch('unknown discriminator', 'not in current catalog or IDL', 'critical');
	}

	function filteredAmountTriggers() {
		return data.protocol.amount_triggers.filter((trigger) =>
			textMatchesSearch(trigger.label, trigger.description)
		);
	}

	function filteredMetricAlertEvents() {
		return metricAlertEvents().filter(eventMatchesSearch);
	}

	function filteredVersions() {
		return data.versions.filter((version) =>
			textMatchesSearch(
				versionLabel(version),
				version.slot,
				version.signature,
				version.loader,
				version.program_sha256,
				version.program_len
			)
		);
	}

	function filteredIdlVersions() {
		return data.idlVersions.filter((version) =>
			textMatchesSearch(
				originLabel(version.origin),
				version.slot,
				version.signature,
				version.kind,
				version.idl_sha256,
				version.idl_len,
				version.compression
			)
		);
	}

	function instructionMapDescription() {
		const rows = instructionFlagRows().length;
		const unknownCopy = data.protocol.unknown_alerts
			? ' Unknown discriminators are treated as critical until reviewed.'
			: '';
		if (rows > 0) {
			return `${rows} cataloged admin or high-impact instruction${rows === 1 ? '' : 's'} for this protocol.${unknownCopy}`;
		}
		return `No cataloged admin instruction map is attached to this program yet.${unknownCopy}`;
	}

	function metricEventTitle(event: ChainEvent) {
		const instruction = eventPayloadString(event, 'instruction_name');
		const metricLabel = eventPayloadString(event, 'metric_label');
		if (instruction && metricLabel) {
			return `${readableInstructionName(instruction)} · ${metricLabel}`;
		}
		return compactLongTokens(event.summary);
	}

	function metricEventValue(event: ChainEvent) {
		const metricValue = eventPayloadNumber(event, 'metric_value');
		if (metricValue === null) return null;
		const previous = eventPayloadNumber(event, 'previous_metric_value');
		if (previous === null) return formatMetricValue(metricValue);
		return `${formatMetricValue(previous)} -> ${formatMetricValue(metricValue)}`;
	}

	function versionBytesUrl(version: ProgramVersionRecord, side: 'before' | 'after') {
		const path = `/api/programs/${data.program.program_id}/versions/${encodeURIComponent(version.id)}/bytes`;
		return `${data.backendBaseUrl}${path}?side=${side}`;
	}

	function idlBytesUrl(version: ProgramIdlVersionRecord) {
		return `${data.backendBaseUrl}/api/programs/${data.program.program_id}/idls/${encodeURIComponent(version.id)}/bytes`;
	}

	function idlTextUrl(version: ProgramIdlVersionRecord) {
		return `${data.backendBaseUrl}/api/programs/${data.program.program_id}/idls/${encodeURIComponent(version.id)}/text`;
	}

	function versionCompareUrl(version: ProgramVersionRecord) {
		return `/programs/${data.program.program_id}/versions/${encodeURIComponent(version.id)}/compare`;
	}

	function downloadName(version: ProgramVersionRecord, side: 'before' | 'after') {
		const label = data.program.label.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
		return `${label || data.program.program_id}-${version.slot}-${side}.so`;
	}

	function idlDownloadName(version: ProgramIdlVersionRecord, extension: 'json' | 'bin') {
		const label = data.program.label.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
		return `${label || data.program.program_id}-${version.slot}-idl.${extension}`;
	}

	function closestProgramVersion(version: ProgramIdlVersionRecord) {
		if (data.versions.length === 0) return null;
		return data.versions.reduce((best, candidate) => {
			if (!best) return candidate;
			return Math.abs(candidate.slot - version.slot) < Math.abs(best.slot - version.slot)
				? candidate
				: best;
		}, null as ProgramVersionRecord | null);
	}

	function programTimingLabel(version: ProgramIdlVersionRecord) {
		const closest = closestProgramVersion(version);
		if (!closest) return null;
		const delta = version.slot - closest.slot;
		if (delta === 0) return 'same slot as program update';
		const distance = new Intl.NumberFormat('en-US').format(Math.abs(delta));
		return delta > 0
			? `${distance} slots after program update`
			: `${distance} slots before program update`;
	}

	function stats() {
		const items = [
			{ label: 'Program', value: shortAddress(data.program.program_id) },
			{ label: 'Versions', value: formatCount(data.versions.length) },
			{ label: 'Last update slot', value: latestUpdateSlotLabel() },
			{ label: 'IDL versions', value: formatCount(data.idlVersions.length) },
			{ label: 'Known instructions', value: formatCount(data.protocol.catalog_instruction_count) },
			{ label: 'Diffs', value: `${diffableVersionCount()}` },
			{ label: 'Loaded events', value: formatCount(allEvents().length) }
		];

		if (!data.catalogedProgram) {
			return [{ label: 'Catalog', value: 'Raw address' }, ...items];
		}
		return items;
	}

	async function loadMore() {
		const offset = currentNextOffset();
		if (loadingMore || offset === null) return;

		loadingMore = true;
		loadMoreError = null;

		try {
			const search = new URLSearchParams({
				'page[size]': '120',
				'page[offset]': String(offset),
				target_address: data.program.program_id,
				scope: 'programs'
			});

			const response = await fetch(`${data.backendBaseUrl}/api/events?${search.toString()}`);
			if (!response.ok) throw new Error(`Backend returned ${response.status}`);

			const payload = (await response.json()) as EventsResponse;
			const seenIds = new Set(allEvents().map((event) => event.id));
			appendedEvents = [...appendedEvents, ...payload.items.filter((event) => !seenIds.has(event.id))];
			nextOffsetOverride = nextPageOffset(payload.links);
		} catch (error) {
			loadMoreError = error instanceof Error ? error.message : 'Unknown backend error';
		} finally {
			loadingMore = false;
		}
	}
</script>

<svelte:head>
	<title>{data.program.label} · FireWatch</title>
</svelte:head>

<div class="page-shell">
	<section class="panel page-top">
		<div class="title-row">
			<ProtocolIdentity
				slug={data.protocol.slug}
				label={data.protocol.name}
				href={protocolHref()}
			/>
			<div class="title-copy">
				<h1>{data.program.label}</h1>
				<p class="lede">{programLede()}</p>
			</div>
		</div>

		<div class="page-actions">
			<a class="secondary-link" href="/programs">All programs</a>
			<a class="primary-link" href={`/events?target=${data.program.program_id}&scope=programs`}>Program activity</a>
			<a class="secondary-link" href={explorerAddressUrl(data.program.program_id, data.explorerCluster)} rel="noreferrer" target="_blank">
				Explorer
			</a>
			{#if data.catalogedProgram}
				<a class="secondary-link" href={protocolPageHref(data.protocol.slug)}>Back to protocol</a>
			{/if}
		</div>

		<div class="stats-strip">
			{#each stats() as stat (stat.label)}
				<span class="stat-item">
					<strong>{stat.value}</strong>
					{stat.label}
				</span>
			{/each}
		</div>

		{#if data.programEventsError}
			<p class="flash error">Activity unavailable: {data.programEventsError}</p>
		{/if}
	</section>

	<section class="panel filter-panel">
		<label class="search-field">
			<span>Filter this program page</span>
			<input
				type="search"
				bind:value={searchQuery}
				placeholder="Search instruction, signature, slot, hash, signer..."
			/>
		</label>
		<div class="filter-chips" aria-label="Activity filters">
			<button class:active={activityFilter === 'all'} type="button" onclick={() => (activityFilter = 'all')}>
				All activity
			</button>
			<button
				class:active={activityFilter === 'critical'}
				type="button"
				onclick={() => (activityFilter = 'critical')}
			>
				Critical
			</button>
			<button class:active={activityFilter === 'high'} type="button" onclick={() => (activityFilter = 'high')}>
				High
			</button>
			<button
				class:active={activityFilter === 'standard'}
				type="button"
				onclick={() => (activityFilter = 'standard')}
			>
				Standard
			</button>
		</div>
	</section>

	<section class="overview-grid">
		<article class="panel">
			<SectionHeader title="Program state" description={latestUpdateDescription()} compact />

			<div class="surface-list">
				<a class="surface-link" href={explorerAddressUrl(data.program.program_id, data.explorerCluster)} rel="noreferrer" target="_blank">
					<strong>Program address</strong>
					<span>{shortAddress(data.program.program_id)}</span>
				</a>
				<div class="surface-card">
					<strong>Latest update slot</strong>
					<span>{latestUpdateSlotLabel()}</span>
				</div>
				<div class="surface-card">
					<strong>Extracted versions</strong>
					<span>{formatCount(data.versions.length)}</span>
				</div>
				{#if latestVersion()}
					{@const version = latestVersion()!}
					<code>{shortDigest(version.program_sha256)}</code>
				{/if}
			</div>
		</article>

		<article class="panel">
			<SectionHeader title="IDL surface" description={idlStateDescription()} compact />

			<div class="surface-list">
				{#if idlAccountLinks().length === 0 && !latestIdlVersion()}
					<p class="empty-state">No IDL account is cataloged for this program.</p>
				{/if}

				{#each idlAccountLinks() as link (link.address)}
					<a class="surface-link" href={link.href} rel="noreferrer" target="_blank">
						<strong>{link.label}</strong>
						<span>{shortAddress(link.address)}</span>
					</a>
				{/each}

				{#if latestIdlVersion()}
					{@const version = latestIdlVersion()!}
					<div class="surface-card">
						<strong>Latest extracted IDL</strong>
						<span>slot {formatCount(version.slot)}</span>
						<code>{shortDigest(version.idl_sha256)}</code>
					</div>
				{/if}
			</div>
		</article>

		<article class="panel">
			<SectionHeader
				title="Squads signers"
				description="Who has signed the most FireWatch-seen actions for this program."
				compact
			/>

			{#if data.signerStatsError}
				<p class="flash error">Signer stats unavailable: {data.signerStatsError}</p>
			{:else if data.signerStats.length === 0}
				<p class="empty-state">No Squads signer history for this program yet.</p>
			{:else}
				<div class="signer-list">
					{#each data.signerStats as signer (signer.member)}
						<div class="signer-row">
							<div class="signer-copy">
								<strong>{signer.member_label ?? shortAddress(signer.member)}</strong>
								<code>{shortAddress(signer.member)}</code>
							</div>
							<div class="signer-metrics">
								<strong>{signer.actions}</strong>
								<span>{signer.multisig_count} multisigs</span>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</article>
	</section>

	<section class="panel instruction-panel">
		<SectionHeader
			title="Instruction monitoring"
			description={instructionMapDescription()}
			compact
		/>

		<div class="instruction-grid">
			<article class="instruction-block">
				<div class="instruction-head">
					<h2>Admin and critical flags</h2>
					<p>Cataloged instructions that FireWatch treats as privileged or high-impact.</p>
				</div>

				{#if filteredInstructionFlagRows().length === 0 && (!data.protocol.unknown_alerts || !showUnknownInstructionRow())}
					<p class="empty-state">No admin or critical instruction flags are cataloged yet.</p>
				{:else}
					<div class="instruction-list">
						{#each filteredInstructionFlagRows() as row (row.name)}
							<div class="instruction-row">
								<div class="instruction-copy">
									<strong>{readableInstructionName(row.name)}</strong>
									<code>{row.name}</code>
								</div>
								<div class="instruction-tags">
									{#if row.highImpact}
										<span class="flag critical">High impact</span>
									{/if}
									{#if row.admin}
										<span class="flag">Admin</span>
									{/if}
									{#if row.loadedEvents > 0}
										<span>{row.loadedEvents} loaded</span>
									{/if}
									{#if row.metricEvents > 0}
										<span>{row.metricEvents} metric</span>
									{/if}
								</div>
							</div>
						{/each}
						{#if data.protocol.unknown_alerts && showUnknownInstructionRow()}
							<div class="instruction-row">
								<div class="instruction-copy">
									<strong>Unknown discriminator</strong>
									<code>not in current catalog or IDL</code>
								</div>
								<div class="instruction-tags">
									<span class="flag critical">Critical</span>
									{#if unknownInstructionEvents().length > 0}
										<span>{unknownInstructionEvents().length} loaded</span>
									{/if}
								</div>
							</div>
						{/if}
					</div>
				{/if}
			</article>

			<article class="instruction-block">
				<div class="instruction-head">
					<h2>Amount and value rules</h2>
					<p>Rules that only alert when a decoded value crosses an abnormal threshold.</p>
				</div>

				{#if filteredAmountTriggers().length === 0 && filteredMetricAlertEvents().length === 0}
					<p class="empty-state">No amount or value rules are cataloged for this protocol yet.</p>
				{:else}
					<div class="instruction-list">
						{#each filteredAmountTriggers() as trigger (`${trigger.label}:${trigger.description}`)}
							<div class="instruction-row">
								<div class="instruction-copy">
									<strong>{trigger.label}</strong>
									<p>{trigger.description}</p>
								</div>
								<div class="instruction-tags">
									<span class="flag">Rule</span>
								</div>
							</div>
						{/each}
						{#each filteredMetricAlertEvents().slice(0, 4) as event (event.id)}
							<a class="instruction-row event-link" href={eventDetailHref(event)}>
								<div class="instruction-copy">
									<strong>{metricEventTitle(event)}</strong>
									<span>{compactLongTokens(event.summary)}</span>
								</div>
								<div class="instruction-tags">
									<span class="flag critical">Observed</span>
									{#if metricEventValue(event)}
										<span>{metricEventValue(event)}</span>
									{/if}
								</div>
							</a>
						{/each}
					</div>
				{/if}
			</article>
		</div>
	</section>

	<section class="panel">
		<SectionHeader
			title="Program versions"
			description={`${data.versions.length} extracted versions from loader uploads, ${diffableVersionCount()} with code diffs.`}
			compact
		/>

		{#if data.versionsError}
			<p class="flash error">Versions unavailable: {data.versionsError}</p>
		{:else if data.versions.length === 0}
			<p class="empty-state">No extracted program versions for this address yet.</p>
		{:else if filteredVersions().length === 0}
			<p class="empty-state">No program versions match the current filter.</p>
		{:else}
			<div class="version-list">
				{#each filteredVersions() as version (version.id)}
					<div class="version-row">
						<div class="version-main">
							<div class="version-title">
								<strong>{versionLabel(version)}</strong>
								{#if version.id === latestVersion()?.id}
									<span class="latest-pill">Latest</span>
								{/if}
								<span>slot {version.slot}</span>
								<span>{formatTimestamp(version.block_time)}</span>
							</div>
							<div class="version-meta">
								<a
									class="version-tx"
									href={explorerTransactionUrl(version.signature, data.explorerCluster)}
									rel="noreferrer"
									target="_blank"
								>
									Tx {shortSignature(version.signature)}
								</a>
								<span>{version.loader}</span>
								<span>{formatByteCount(version.program_len)} bytes</span>
								<span>{version.chunk_count} chunks</span>
							</div>
							<code>{shortDigest(version.program_sha256)}</code>
						</div>
						<div class="version-actions">
							{#if canCompareVersion(version)}
								<a class="primary-link compact-link" href={versionCompareUrl(version)}>Code diff</a>
							{/if}
							{#if version.previous_version_id}
								<a
									class="secondary-link compact-link"
									href={versionBytesUrl(version, 'before')}
									download={downloadName(version, 'before')}
								>
									Before
								</a>
							{/if}
							{#if version.complete}
								<a
									class="secondary-link compact-link"
									href={versionBytesUrl(version, 'after')}
									download={downloadName(version, 'after')}
								>
									After
								</a>
							{:else}
								<span class="version-state">Incomplete</span>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</section>

	{#if data.idlVersionsError || data.idlVersions.length > 0}
		<section class="panel">
			<SectionHeader
				title="IDL versions"
				description={`${data.idlVersions.length} on-chain IDL versions from Program Metadata and legacy Anchor storage.`}
				compact
			/>

			{#if data.idlVersionsError}
				<p class="flash error">IDL versions unavailable: {data.idlVersionsError}</p>
			{:else if filteredIdlVersions().length === 0}
				<p class="empty-state">No IDL versions match the current filter.</p>
			{:else}
				<div class="version-list">
					{#each filteredIdlVersions() as version (version.id)}
						<div class="version-row">
							<div class="version-main">
								<div class="version-title">
									<strong>{originLabel(version.origin)}</strong>
									<span>slot {version.slot}</span>
									<span>{formatTimestamp(version.block_time)}</span>
									{#if programTimingLabel(version)}
										<span>{programTimingLabel(version)}</span>
									{/if}
								</div>
								<div class="version-meta">
									<a
										class="version-tx"
										href={explorerTransactionUrl(version.signature, data.explorerCluster)}
										rel="noreferrer"
										target="_blank"
									>
										Tx {shortSignature(version.signature)}
									</a>
									<span>{version.kind.replace('program_idl_', '').replaceAll('_', ' ')}</span>
									<span>{formatByteCount(version.idl_len)} bytes</span>
									<span>{version.chunk_count} chunks</span>
									<span>{version.compression ?? 'raw'}</span>
								</div>
								<code>{shortDigest(version.idl_sha256)}</code>
							</div>
							<div class="version-actions">
								{#if version.complete}
									{#if version.format === 'json'}
										<a
											class="secondary-link compact-link"
											href={idlTextUrl(version)}
											download={idlDownloadName(version, 'json')}
										>
											JSON
										</a>
									{/if}
									<a
										class="primary-link compact-link"
										href={idlBytesUrl(version)}
										download={idlDownloadName(version, 'bin')}
									>
										Bytes
									</a>
								{:else}
									<span class="version-state">Incomplete</span>
								{/if}
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</section>
	{/if}

	<section class="panel">
		<SectionHeader
			title="Recent activity"
			description={`${filteredEvents().length} of ${allEvents().length} loaded events match.`}
			href={`/events?target=${data.program.program_id}&scope=programs`}
			hrefLabel="Open full activity"
			compact
		/>

		{#if allEvents().length === 0}
			<p class="empty-state">No recent events on this program yet.</p>
		{:else if filteredEvents().length === 0}
			<p class="empty-state">No loaded events match the current filter.</p>
		{:else}
			<div class="event-list">
				{#each filteredEvents() as event (event.id)}
					<ActivityEventRow
						{event}
						explorerCluster={data.explorerCluster}
						protocols={[data.protocol]}
						fallbackProtocolSlug={data.protocol.slug}
						fallbackProtocolLabel={data.protocol.name}
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
	.surface-list,
	.event-list,
	.version-list,
	.signer-list {
		display: grid;
		gap: 0.68rem;
	}

	.title-row {
		grid-template-columns: auto minmax(0, 1fr);
		align-items: start;
		min-width: 0;
	}

	h1,
	h2,
	strong,
	p {
		margin: 0;
	}

	h1 {
		font-size: 1.8rem;
		line-height: 1.02;
		letter-spacing: 0;
		overflow-wrap: anywhere;
	}

	h2 {
		font-size: 0.96rem;
		line-height: 1.25;
	}

	.lede,
	.empty-state,
	.flash,
	.instruction-head p,
	.instruction-copy p,
	.instruction-copy span,
	.signer-row span {
		color: #5f6c79;
	}

	.lede {
		max-width: 60ch;
		line-height: 1.5;
	}

	.page-actions,
	.stats-strip,
	.load-row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.52rem;
	}

	.filter-panel {
		display: grid;
		grid-template-columns: minmax(18rem, 1fr) auto;
		align-items: end;
		gap: 0.76rem;
	}

	.search-field {
		display: grid;
		gap: 0.28rem;
		min-width: 0;
	}

	.search-field span {
		color: #5f6c79;
		font-size: 0.78rem;
		font-weight: 800;
	}

	.search-field input {
		width: 100%;
		min-height: 2.5rem;
		padding: 0 0.76rem;
		border: 1px solid rgba(23, 32, 42, 0.12);
		border-radius: 8px;
		background: #fff;
		color: #17202a;
		font: inherit;
		font-size: 0.9rem;
	}

	.search-field input:focus {
		outline: 2px solid rgba(189, 74, 23, 0.18);
		border-color: rgba(189, 74, 23, 0.55);
	}

	.filter-chips {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: 0.42rem;
	}

	.filter-chips button {
		min-height: 2.2rem;
		padding: 0 0.68rem;
		border: 1px solid rgba(23, 32, 42, 0.12);
		border-radius: 8px;
		background: #fff;
		color: #556473;
		font: inherit;
		font-size: 0.8rem;
		font-weight: 800;
		cursor: pointer;
	}

	.filter-chips button.active {
		border-color: #bd4a17;
		background: #bd4a17;
		color: #fff;
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
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 0.76rem;
	}

	.instruction-panel,
	.instruction-block,
	.instruction-head,
	.instruction-list,
	.instruction-copy {
		display: grid;
		gap: 0.56rem;
	}

	.instruction-grid {
		display: grid;
		grid-template-columns: minmax(0, 1.15fr) minmax(0, 0.85fr);
		gap: 0.76rem;
		align-items: start;
	}

	.signer-row,
	.version-row,
	.instruction-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: start;
		gap: 0.72rem;
		padding: 0.72rem 0.78rem;
		border-radius: 8px;
		background: #fbfaf8;
		border: 1px solid rgba(23, 32, 42, 0.08);
	}

	.version-list {
		grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
		align-items: stretch;
	}

	.version-row {
		grid-template-columns: minmax(0, 1fr);
		align-content: space-between;
		min-height: 11rem;
	}

	.version-actions {
		margin-top: 0.12rem;
	}

	.signer-copy,
	.version-main {
		display: grid;
		gap: 0.18rem;
		min-width: 0;
	}

	.version-title,
	.version-meta,
	.version-actions {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0.42rem;
		min-width: 0;
	}

	.version-title span,
	.version-meta,
	.version-state {
		color: #66737f;
		font-size: 0.78rem;
		font-weight: 600;
	}

	.version-title .latest-pill {
		padding: 0.12rem 0.34rem;
		border: 1px solid rgba(189, 74, 23, 0.18);
		border-radius: 6px;
		background: rgba(255, 123, 54, 0.12);
		color: #8d421a;
		font-size: 0.7rem;
		text-transform: uppercase;
	}

	.version-title strong {
		font-size: 0.9rem;
	}

	.version-meta {
		gap: 0.52rem;
	}

	.version-tx {
		color: #4e5d6b;
		text-decoration: underline;
		text-decoration-color: rgba(23, 32, 42, 0.2);
		text-underline-offset: 0.14em;
	}

	.version-state {
		padding: 0.5rem 0.64rem;
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 8px;
		background: #fff;
	}

	.surface-list {
		grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
	}

	.surface-link,
	.surface-card {
		display: grid;
		gap: 0.18rem;
		padding: 0.72rem 0.78rem;
		border-radius: 8px;
		background: #fbfaf8;
		border: 1px solid rgba(23, 32, 42, 0.08);
		text-decoration: none;
	}

	.surface-card {
		min-width: 0;
	}

	.surface-link strong,
	.surface-card strong {
		font-size: 0.72rem;
		font-weight: 600;
		color: #758492;
	}

	.surface-link span,
	.surface-card span {
		font-size: 0.84rem;
		font-weight: 700;
		color: #1d2730;
		overflow-wrap: anywhere;
	}

	.instruction-head p,
	.instruction-copy p,
	.instruction-copy span {
		font-size: 0.82rem;
		line-height: 1.45;
	}

	.instruction-row {
		text-decoration: none;
		color: inherit;
	}

	.instruction-tags {
		display: flex;
		flex-wrap: wrap;
		gap: 0.32rem;
		justify-content: flex-end;
		color: #66737f;
		font-size: 0.76rem;
		font-weight: 700;
	}

	.flag {
		display: inline-flex;
		align-items: center;
		min-height: 1.36rem;
		padding: 0.08rem 0.38rem;
		border-radius: 6px;
		border: 1px solid rgba(23, 32, 42, 0.1);
		background: #fff;
		color: #556473;
	}

	.flag.critical {
		border-color: rgba(156, 57, 41, 0.22);
		background: rgba(191, 74, 57, 0.1);
		color: #9c3929;
	}

	.event-link:hover strong {
		text-decoration: underline;
		text-underline-offset: 0.14em;
	}

	code {
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
		font-size: 0.8rem;
		color: #63707c;
		overflow-wrap: anywhere;
	}

	.signer-metrics {
		display: grid;
		gap: 0.08rem;
		text-align: right;
		min-width: 0;
	}

	.primary-link,
	.secondary-link {
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

	.compact-link {
		padding: 0.5rem 0.64rem;
		font-size: 0.8rem;
	}

	.primary-link {
		border: 1px solid #bd4a17;
		background: #bd4a17;
		color: white;
	}

	.secondary-link {
		background: #fff;
		color: #556473;
		border: 1px solid rgba(23, 32, 42, 0.12);
		cursor: pointer;
	}

	.button-like {
		cursor: pointer;
	}

	.flash {
		padding: 0.74rem 0.82rem;
		border-radius: 8px;
		font-size: 0.88rem;
		font-weight: 600;
	}

	.flash.error {
		background: rgba(191, 74, 57, 0.14);
		color: #9c3929;
	}

	.load-row {
		align-items: center;
	}

	@media (max-width: 980px) {
		.overview-grid,
		.instruction-grid,
		.filter-panel {
			grid-template-columns: 1fr;
		}

		.filter-chips {
			justify-content: flex-start;
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

		.signer-row,
		.version-row,
		.instruction-row {
			grid-template-columns: 1fr;
		}

		.instruction-tags {
			justify-content: flex-start;
		}

		.version-actions {
			justify-content: flex-start;
		}
	}
</style>
