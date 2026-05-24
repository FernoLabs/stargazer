<script lang="ts">
	import { explorerTransactionUrl } from '$lib/explorer';
	import ProgramInstructionChanges from '$lib/components/ProgramInstructionChanges.svelte';
	import ProgramPatchView from '$lib/components/ProgramPatchView.svelte';
	import ProgramVersionDiff from '$lib/components/ProgramVersionDiff.svelte';
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import { formatTimestamp, shortAddress } from '$lib/format';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	function isLargeAssembly() {
		return (
			(data.beforeDisassembly?.instruction_count ?? 0) > 100_000 ||
			(data.afterDisassembly?.instruction_count ?? 0) > 100_000
		);
	}

	let mode = $state<'changes' | 'patch' | 'assembly' | 'hex'>(
		isLargeAssembly() ? 'assembly' : 'changes'
	);
	let beforeHex = $state<string | null>(null);
	let afterHex = $state<string | null>(null);
	let hexLoading = $state(false);
	let hexError = $state<string | null>(null);
	let ignoreRelocationNoise = $state(true);

	function versionBytesUrl(side: 'before' | 'after') {
		const path = `/api/programs/${data.program.program_id}/versions/${encodeURIComponent(data.version.id)}/bytes`;
		return `${data.backendBaseUrl}${path}?side=${side}`;
	}

	function compareHref(address: string) {
		return `/programs/${address}`;
	}

	function shortDigest(value: string | null) {
		if (!value) return 'Pending';
		if (value.length <= 18) return value;
		return `${value.slice(0, 8)}...${value.slice(-8)}`;
	}

	function afterTitle() {
		return `${data.version.kind === 'program_deployed' ? 'Deploy' : 'Upgrade'} at slot ${data.version.slot}`;
	}

	function beforeTitle() {
		return data.version.previous_version_id ? 'Previous version' : 'No previous version';
	}

	function assemblyBeforeText() {
		return data.beforeDisassembly?.asm_text ?? '';
	}

	function assemblyAfterText() {
		return data.afterDisassembly?.asm_text ?? '';
	}

	function activeBeforeText() {
		return mode === 'hex' ? beforeHex ?? '' : assemblyBeforeText();
	}

	function activeAfterText() {
		return mode === 'hex' ? afterHex ?? '' : assemblyAfterText();
	}

	function printable(byte: number) {
		return byte >= 32 && byte <= 126 ? String.fromCharCode(byte) : '.';
	}

	function hexDump(bytes: Uint8Array) {
		const rows: string[] = [];
		for (let offset = 0; offset < bytes.length; offset += 16) {
			const chunk = bytes.slice(offset, offset + 16);
			const hex = Array.from(chunk, (byte) => byte.toString(16).padStart(2, '0'))
				.join(' ')
				.padEnd(47, ' ');
			const ascii = Array.from(chunk, printable).join('');
			rows.push(`${offset.toString(16).padStart(8, '0')}  ${hex}  |${ascii}|`);
		}
		return rows.join('\n');
	}

	async function fetchHex(side: 'before' | 'after') {
		const response = await fetch(versionBytesUrl(side));
		if (!response.ok) {
			if (response.status === 404 && side === 'before') return '';
			throw new Error(`Backend returned ${response.status}`);
		}
		return hexDump(new Uint8Array(await response.arrayBuffer()));
	}

	function downloadName(side: 'before' | 'after') {
		const label = data.program.label.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
		return `${label || data.program.program_id}-${data.version.slot}-${side}.so`;
	}

	function patchBeforeLabel() {
		return data.version.previous_version_id
			? `${data.program.label} previous`
			: `${data.program.label} empty`;
	}

	function patchAfterLabel() {
		return `${data.program.label} slot ${data.version.slot}`;
	}

	async function setMode(nextMode: 'changes' | 'patch' | 'assembly' | 'hex') {
		mode = nextMode;
		if (nextMode !== 'hex' || (beforeHex !== null && afterHex !== null)) return;

		hexLoading = true;
		hexError = null;
		try {
			const [before, after] = await Promise.all([
				data.version.previous_version_id ? fetchHex('before') : Promise.resolve(''),
				fetchHex('after')
			]);
			beforeHex = before;
			afterHex = after;
		} catch (err) {
			hexError = err instanceof Error ? err.message : 'Unknown backend error';
		} finally {
			hexLoading = false;
		}
	}
</script>

<svelte:head>
	<title>{data.program.label} version diff · FireWatch</title>
</svelte:head>

<div class="page-shell">
	<section class="panel">
		<div class="top-row">
			<SectionHeader
				title="Version diff"
				description={`${afterTitle()} for ${data.program.label}.`}
				compact
			/>
			<div class="top-actions">
				<a class="secondary-link" href={compareHref(data.program.program_id)}>Back to versions</a>
				<a
					class="secondary-link"
					href={explorerTransactionUrl(data.version.signature, data.explorerCluster)}
					rel="noreferrer"
					target="_blank"
				>
					Open tx
				</a>
				{#if data.version.previous_version_id}
					<a
						class="secondary-link"
						href={versionBytesUrl('before')}
						download={downloadName('before')}
					>
						Before bytes
					</a>
				{/if}
				<a class="secondary-link" href={versionBytesUrl('after')} download={downloadName('after')}>
					After bytes
				</a>
			</div>
		</div>

		<div class="version-summary">
			<div>
				<span>Before</span>
				<strong>{beforeTitle()}</strong>
				<code>{shortDigest(data.version.previous_program_sha256)}</code>
			</div>
			<div>
				<span>After</span>
				<strong>slot {data.version.slot}</strong>
				<code>{shortDigest(data.version.program_sha256)}</code>
			</div>
			<div>
				<span>Updated</span>
				<strong>{formatTimestamp(data.version.block_time)}</strong>
				<code>{shortAddress(data.version.signature)}</code>
			</div>
		</div>
	</section>

	<section class="panel viewer-panel">
		<div class="viewer-toolbar">
			<div class="mode-tabs" role="tablist" aria-label="Diff mode">
				<button class:active={mode === 'changes'} type="button" onclick={() => setMode('changes')}>
					Changed rows
				</button>
				<button class:active={mode === 'patch'} type="button" onclick={() => setMode('patch')}>
					Patch
				</button>
				<button class:active={mode === 'assembly'} type="button" onclick={() => setMode('assembly')}>
					Split diff
				</button>
				<button class:active={mode === 'hex'} type="button" onclick={() => setMode('hex')}>
					Hex
				</button>
			</div>
			<div class="viewer-stats">
				{#if isLargeAssembly()}
					<span>Large program diff</span>
				{/if}
				{#if mode === 'assembly' || mode === 'changes' || mode === 'patch'}
					<span>{data.beforeDisassembly?.instruction_count ?? 0} before instructions</span>
					<span>{data.afterDisassembly?.instruction_count ?? 0} after instructions</span>
				{:else if hexLoading}
					<span>Loading hex view...</span>
				{:else}
					<span>{data.version.previous_program_len ?? 0} before bytes</span>
					<span>{data.version.program_len ?? 0} after bytes</span>
				{/if}
			</div>
			{#if mode === 'changes' && !isLargeAssembly()}
				<label class="toolbar-check">
					<input type="checkbox" bind:checked={ignoreRelocationNoise} />
					Ignore relocated targets
				</label>
			{/if}
		</div>

		{#if data.disassemblyError && mode !== 'hex'}
			<p class="flash error">Disassembly unavailable: {data.disassemblyError}</p>
		{:else if hexError && mode === 'hex'}
			<p class="flash error">Hex view unavailable: {hexError}</p>
		{:else if hexLoading && mode === 'hex'}
			<p class="empty-state">Loading binary hex view.</p>
		{:else if mode === 'changes' && isLargeAssembly()}
			<div class="viewer-note">
				<p>
					Changed rows are unavailable above 100,000 instructions. Use split diff or hex for this version.
				</p>
			</div>
		{:else if mode === 'changes'}
			<ProgramInstructionChanges
				beforeText={assemblyBeforeText()}
				afterText={assemblyAfterText()}
				{ignoreRelocationNoise}
			/>
		{:else if mode === 'patch' && isLargeAssembly()}
			<div class="viewer-note">
				<p>Patch view is unavailable above 100,000 instructions. Use split diff or hex for this version.</p>
			</div>
		{:else if mode === 'patch'}
			<ProgramPatchView
				beforeText={assemblyBeforeText()}
				afterText={assemblyAfterText()}
				beforeLabel={patchBeforeLabel()}
				afterLabel={patchAfterLabel()}
			/>
		{:else}
			<ProgramVersionDiff
				beforeText={activeBeforeText()}
				afterText={activeAfterText()}
				beforeLabel={mode === 'hex' ? 'Before bytes' : patchBeforeLabel()}
				afterLabel={mode === 'hex' ? 'After bytes' : patchAfterLabel()}
				mode={mode === 'hex' ? 'hex' : 'assembly'}
			/>
		{/if}
	</section>
</div>

<style>
	.page-shell {
		width: min(1680px, calc(100vw - 1.25rem));
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

	.top-row,
	.viewer-toolbar {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 0.76rem;
		flex-wrap: wrap;
	}

	.top-actions,
	.mode-tabs,
	.viewer-stats,
	.toolbar-check {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 0.5rem;
	}

	.version-summary {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 1px;
		margin-top: 0.82rem;
		overflow: hidden;
		border-radius: 8px;
		background: var(--fw-border);
	}

	.version-summary div {
		display: grid;
		gap: 0.14rem;
		min-width: 0;
		padding: 0.68rem 0.74rem;
		background: var(--fw-panel-soft);
	}

	.version-summary span,
	.viewer-stats {
		color: #697786;
		font-size: 0.78rem;
		font-weight: 600;
	}

	.version-summary strong {
		color: #1c2731;
		font-size: 0.9rem;
		overflow-wrap: anywhere;
	}

	code {
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
		font-size: 0.78rem;
		color: #63707c;
		overflow-wrap: anywhere;
	}

	.secondary-link,
	.mode-tabs button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 0.58rem 0.78rem;
		border-radius: 8px;
		font-size: 0.84rem;
		font-weight: 600;
		text-decoration: none;
		background: #fff;
		color: #556473;
		border: 1px solid rgba(23, 32, 42, 0.12);
		cursor: pointer;
	}

	.mode-tabs button.active {
		color: #fff;
		border-color: #bd4a17;
		background: #bd4a17;
	}

	.viewer-panel {
		display: grid;
		gap: 0.76rem;
	}

	.viewer-toolbar {
		position: sticky;
		top: 4.72rem;
		z-index: 4;
		padding-bottom: 0.76rem;
		background: #fff;
		border-bottom: 1px solid rgba(23, 32, 42, 0.08);
	}

	.flash,
	.viewer-note,
	.empty-state {
		margin: 0;
		padding: 0.74rem 0.82rem;
		border-radius: 8px;
		font-size: 0.88rem;
		font-weight: 600;
	}

	.empty-state {
		color: #5f6c79;
	}

	.viewer-note {
		background: #fbfaf8;
		color: #5f6c79;
		border: 1px solid rgba(23, 32, 42, 0.08);
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		flex-wrap: wrap;
	}

	.viewer-note p {
		margin: 0;
		max-width: 82ch;
	}

	.toolbar-check {
		gap: 0.42rem;
		color: #4d5b68;
		font-size: 0.82rem;
		font-weight: 700;
		white-space: nowrap;
	}

	.flash.error {
		background: rgba(191, 74, 57, 0.14);
		color: #9c3929;
	}

	@media (max-width: 760px) {
		.page-shell {
			width: min(100vw - 1rem, var(--fw-page-max));
		}

		.version-summary {
			grid-template-columns: 1fr;
		}
	}
</style>
