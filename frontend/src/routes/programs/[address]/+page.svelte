<script lang="ts">
	import { explorerAddressUrl, explorerIdlUrl, explorerTransactionUrl } from '$lib/explorer';
	import { protocolPageHref } from '$lib/catalog';
	import { watchProgramCommand } from '$lib/bot-commands';
	import ActivityEventRow from '$lib/components/ActivityEventRow.svelte';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import { formatTimestamp, shortAddress } from '$lib/format';
	import { programCriticalHref, programRiskSummary } from '$lib/risk';
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
	let copiedState = $state<'idle' | 'copied' | 'error'>('idle');

	function allEvents() {
		if (appendedEvents.length === 0) return data.events;
		const seen = new Set(data.events.map((event) => event.id));
		return [...data.events, ...appendedEvents.filter((event) => !seen.has(event.id))];
	}

	function currentNextOffset() {
		return nextOffsetOverride === undefined ? data.nextOffset : nextOffsetOverride;
	}

	function command() {
		return watchProgramCommand(data.program.program_id);
	}

	function protocolHref() {
		return data.catalogedProgram ? protocolPageHref(data.protocol.slug) : null;
	}

	function programLede() {
		if (data.catalogedProgram) {
			return 'A single watched address for upgrades, IDL changes, admin paths, signer history, and uncommon instruction review.';
		}
		return 'A watched program address with loader-uploaded versions, upgrade activity, signer history, and raw byte downloads.';
	}

	function programDetailsDescription() {
		if (data.program.idl_metadata_address || data.program.anchor_legacy_idl_address) {
			return 'Explorer links for this program and its on-chain IDL accounts.';
		}
		return 'Explorer links for this exact program address.';
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
		const risk = programRiskSummary(data.program);
		const items = [
			{ label: 'Program', value: shortAddress(data.program.program_id) },
			{ label: 'Loaded', value: String(allEvents().length) }
		];

		if (!data.catalogedProgram) {
			return [{ label: 'Catalog', value: 'Raw address' }, ...items];
		}

		return [
			...items.slice(0, 1),
			{ label: 'Rug check', value: risk.grade },
			{ label: 'Score', value: `${risk.score}` },
			{ label: 'Critical', value: `${risk.critical_total}` },
			{ label: 'Unreviewed', value: `${risk.unreviewed_critical}` },
			{ label: 'Reviewed', value: `${risk.reviewed_critical}` },
			...items.slice(1)
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
			const search = new URLSearchParams({
				limit: '120',
				offset: String(offset),
				target_address: data.program.program_id,
				scope: 'programs'
			});

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
			<a class="secondary-link" href={programCriticalHref(data.program.program_id)}>Critical review</a>
			{#if data.catalogedProgram}
				<a class="secondary-link" href={protocolPageHref(data.protocol.slug)}>Back to protocol</a>
			{/if}
		</div>

			<div class="stats-strip">
				{#each stats() as stat}
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

	<section class="overview-grid">
		<article class="panel">
			<SectionHeader
				title="Custom Plan draft"
				description="Use this when a Custom Plan user or eligible member wants to track only this program."
				compact
			/>

			<div class="command-row">
				<div class="command-copy">
					<strong>{data.program.label}</strong>
					<code>{shortAddress(data.program.program_id)}</code>
					<code class="command-code">{command()}</code>
				</div>
				<button class="ghost-button" type="button" onclick={() => copyText(command())}>Copy draft</button>
			</div>

			{#if copiedState === 'copied'}
				<p class="flash success">Setup draft copied.</p>
			{:else if copiedState === 'error'}
				<p class="flash error">Clipboard access failed. Copy the command manually.</p>
			{/if}
		</article>

		<article class="panel">
			<SectionHeader
				title="Program details"
				description={programDetailsDescription()}
				compact
			/>

			<div class="surface-list">
				<a class="surface-link" href={explorerAddressUrl(data.program.program_id, data.explorerCluster)} rel="noreferrer" target="_blank">
					<strong>Program</strong>
					<span>{shortAddress(data.program.program_id)}</span>
				</a>
				{#if data.program.idl_metadata_address}
					<a class="surface-link" href={explorerIdlUrl(data.program.program_id, data.explorerCluster)} rel="noreferrer" target="_blank">
						<strong>Program Metadata IDL</strong>
						<span>{shortAddress(data.program.idl_metadata_address)}</span>
					</a>
				{/if}
				{#if data.program.anchor_legacy_idl_address}
					<a class="surface-link" href={explorerAddressUrl(data.program.anchor_legacy_idl_address, data.explorerCluster)} rel="noreferrer" target="_blank">
						<strong>Anchor legacy IDL</strong>
						<span>{shortAddress(data.program.anchor_legacy_idl_address)}</span>
					</a>
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
					{#each data.signerStats as signer}
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
		{:else}
			<div class="version-list">
				{#each data.versions as version, index}
					<div class="version-row">
						<div class="version-main">
							<div class="version-title">
								<strong>{versionLabel(version)}</strong>
								{#if index === 0}
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

	<section class="panel">
		<SectionHeader
			title="IDL versions"
			description={`${data.idlVersions.length} on-chain IDL versions from Program Metadata and legacy Anchor storage.`}
			compact
		/>

		{#if data.idlVersionsError}
			<p class="flash error">IDL versions unavailable: {data.idlVersionsError}</p>
		{:else if data.idlVersions.length === 0}
			<p class="empty-state">No extracted on-chain IDL versions for this address yet.</p>
		{:else}
			<div class="version-list">
				{#each data.idlVersions as version}
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

	<section class="panel">
		<SectionHeader
			title="Recent activity"
			description={`${allEvents().length} recent events on this program.`}
			href={`/events?target=${data.program.program_id}&scope=programs`}
			hrefLabel="Open full activity"
			compact
		/>

		{#if allEvents().length === 0}
			<p class="empty-state">No recent events on this program yet.</p>
		{:else}
			<div class="event-list">
				{#each allEvents() as event}
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

	.lede,
	.empty-state,
	.flash,
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

	.command-row,
	.signer-row,
	.version-row {
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

	.surface-link {
		display: grid;
		gap: 0.18rem;
		padding: 0.72rem 0.78rem;
		border-radius: 8px;
		background: #fbfaf8;
		border: 1px solid rgba(23, 32, 42, 0.08);
		text-decoration: none;
	}

	.surface-link strong {
		font-size: 0.72rem;
		font-weight: 600;
		color: #758492;
	}

	.surface-link span {
		font-size: 0.84rem;
		font-weight: 700;
		color: #1d2730;
		overflow-wrap: anywhere;
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

	.signer-metrics {
		display: grid;
		gap: 0.08rem;
		text-align: right;
		min-width: 0;
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

	.compact-link {
		padding: 0.5rem 0.64rem;
		font-size: 0.8rem;
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

	@media (max-width: 980px) {
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
		.signer-row,
		.version-row {
			grid-template-columns: 1fr;
		}

		.version-actions {
			justify-content: flex-start;
		}
	}
</style>
