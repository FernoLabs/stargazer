<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import type { MergeView as MergeViewInstance } from '@codemirror/merge';

	type AssemblyLine = {
		number: number | null;
		body: string;
	};

	type AssemblyRow = {
		kind: 'context' | 'changed' | 'removed' | 'added' | 'gap';
		beforeNumber: number | null;
		afterNumber: number | null;
		beforeText: string;
		afterText: string;
		count?: number;
	};

	type CodeMirrorModules = {
		MergeView: typeof import('@codemirror/merge').MergeView;
		EditorState: typeof import('@codemirror/state').EditorState;
		EditorView: typeof import('@codemirror/view').EditorView;
		lineNumbers: typeof import('@codemirror/view').lineNumbers;
	};

	let {
		beforeText,
		afterText,
		beforeLabel = 'Before',
		afterLabel = 'After',
		mode
	}: {
		beforeText: string;
		afterText: string;
		beforeLabel?: string;
		afterLabel?: string;
		mode: 'assembly' | 'hex';
	} = $props();

	let container = $state<HTMLDivElement | undefined>();
	let mergeView: MergeViewInstance | null = null;
	let mounted = false;
	let assemblyRows = $state<AssemblyRow[]>([]);
	let assemblyChangedCount = $state(0);
	let assemblyPending = $state(true);
	let codeMirrorPending = $state(false);
	let codeMirrorLoadError = $state<string | null>(null);
	let assemblyRenderToken = 0;
	let codeMirrorModulesPromise: Promise<CodeMirrorModules> | null = null;

	function loadCodeMirrorModules() {
		codeMirrorModulesPromise ??= Promise.all([
			import('@codemirror/merge'),
			import('@codemirror/state'),
			import('@codemirror/view')
		]).then(([merge, state, view]) => ({
			MergeView: merge.MergeView,
			EditorState: state.EditorState,
			EditorView: view.EditorView,
			lineNumbers: view.lineNumbers
		}));

		return codeMirrorModulesPromise;
	}

	function editorExtensions(modules: CodeMirrorModules) {
		const editorTheme = modules.EditorView.theme({
			'&': {
				height: '72vh',
				fontSize: '12.5px',
				backgroundColor: '#fff'
			},
			'.cm-scroller': {
				fontFamily: "'IBM Plex Mono', 'SFMono-Regular', monospace",
				lineHeight: '1.45'
			},
			'.cm-gutters': {
				backgroundColor: '#f7f6f3',
				color: '#7a8793',
				borderRight: '1px solid rgba(23, 32, 42, 0.1)'
			},
			'.cm-line': {
				padding: '0 0.7rem'
			},
			'.cm-content': {
				padding: '0.5rem 0',
				minWidth: 'max-content'
			}
		});

		return [
			modules.lineNumbers(),
			modules.EditorState.readOnly.of(true),
			modules.EditorView.editable.of(false),
			editorTheme
		];
	}

	function parseAssembly(text: string) {
		return text
			.split('\n')
			.filter((line) => line.length > 0)
			.map((raw): AssemblyLine => {
				const match = /^(\d+):\s?(.*)$/.exec(raw);
				return {
					number: match ? Number(match[1]) : null,
					body: match?.[2] ?? raw
				};
			});
	}

	function allAssemblyRows() {
		const before = parseAssembly(beforeText);
		const after = parseAssembly(afterText);
		const rows: AssemblyRow[] = [];

		for (let index = 0; index < Math.max(before.length, after.length); index += 1) {
			const beforeLine = before[index];
			const afterLine = after[index];
			rows.push({
				kind: beforeLine && afterLine ? (beforeLine.body === afterLine.body ? 'context' : 'changed') : beforeLine ? 'removed' : 'added',
				beforeNumber: beforeLine?.number ?? null,
				afterNumber: afterLine?.number ?? null,
				beforeText: beforeLine?.body ?? '',
				afterText: afterLine?.body ?? ''
			});
		}

		return rows;
	}

	function visibleAssemblyRows(rows: AssemblyRow[]) {
		const contextLines = 2;
		const changed = rows
			.map((row, index) => (row.kind === 'context' ? -1 : index))
			.filter((index) => index >= 0);
		const keep = new Set<number>();

		for (const index of changed) {
			const start = Math.max(0, index - contextLines);
			const end = Math.min(rows.length - 1, index + contextLines);
			for (let current = start; current <= end; current += 1) {
				keep.add(current);
			}
		}

		const output: AssemblyRow[] = [];
		let omitted = 0;
		for (let index = 0; index < rows.length; index += 1) {
			if (!keep.has(index)) {
				omitted += 1;
				continue;
			}
			if (omitted > 0) {
				output.push({
					kind: 'gap',
					beforeNumber: null,
					afterNumber: null,
					beforeText: '',
					afterText: '',
					count: omitted
				});
				omitted = 0;
			}
			output.push(rows[index]);
		}

		return output;
	}

	function renderAssemblyDiff() {
		const token = ++assemblyRenderToken;
		assemblyPending = true;
		assemblyRows = [];
		window.setTimeout(() => {
			if (token !== assemblyRenderToken || mode !== 'assembly') return;
			const rows = allAssemblyRows();
			assemblyChangedCount = rows.filter((row) => row.kind !== 'context').length;
			assemblyRows = visibleAssemblyRows(rows);
			assemblyPending = false;
		}, 0);
	}

	async function renderCodeMirrorDiff() {
		if (!mounted || !container) return;
		const token = ++assemblyRenderToken;
		codeMirrorPending = true;
		codeMirrorLoadError = null;
		mergeView?.destroy();
		container.innerHTML = '';

		try {
			const modules = await loadCodeMirrorModules();
			if (token !== assemblyRenderToken || !mounted || mode === 'assembly' || !container) return;

			mergeView = new modules.MergeView({
				a: {
					doc: beforeText || '',
					extensions: editorExtensions(modules)
				},
				b: {
					doc: afterText || '',
					extensions: editorExtensions(modules)
				},
				parent: container,
				collapseUnchanged: {
					margin: 2,
					minSize: 12
				},
				diffConfig: {
					scanLimit: 500
				}
			});
		} catch (error) {
			if (token === assemblyRenderToken) {
				codeMirrorLoadError =
					error instanceof Error ? error.message : 'Could not load the split editor.';
			}
		} finally {
			if (token === assemblyRenderToken) {
				codeMirrorPending = false;
			}
		}
	}

	function renderActiveView() {
		if (!mounted) return;
		if (mode === 'assembly') {
			mergeView?.destroy();
			mergeView = null;
			renderAssemblyDiff();
			return;
		}
		void renderCodeMirrorDiff();
	}

	function assemblyLineNumber(value: number | null) {
		return value === null ? '' : String(value).padStart(6, '0');
	}

	$effect(() => {
		beforeText;
		afterText;
		mode;
		renderActiveView();
	});

	onMount(() => {
		mounted = true;
		renderActiveView();
	});

	onDestroy(() => {
		assemblyRenderToken += 1;
		mergeView?.destroy();
	});
</script>

<div class="diff-shell">
	<div class="diff-labels" aria-hidden="true">
		<span>{beforeLabel}</span>
		<span>{afterLabel}</span>
	</div>
	{#if mode === 'assembly'}
		<div class="assembly-diff-host" aria-label="Program assembly split diff">
			{#if assemblyPending}
				<p class="empty-state">Preparing changed instruction rows.</p>
			{:else if assemblyRows.length === 0}
				<p class="empty-state">No changed assembly rows after filters.</p>
			{:else}
				<div class="assembly-summary">{assemblyChangedCount} changed instruction rows</div>
				<div class="assembly-grid">
					{#each assemblyRows as row}
						{#if row.kind === 'gap'}
							<div class="assembly-gap-row">
								<span>{row.count} unchanged lines</span>
							</div>
						{:else}
							<div class={`assembly-row ${row.kind}`}>
								<div class="assembly-cell before-cell">
									<code class="asm-line-no">{assemblyLineNumber(row.beforeNumber)}</code>
									<code class="asm-text">{row.beforeText}</code>
								</div>
								<div class="assembly-cell after-cell">
									<code class="asm-line-no">{assemblyLineNumber(row.afterNumber)}</code>
									<code class="asm-text">{row.afterText}</code>
								</div>
							</div>
						{/if}
					{/each}
				</div>
			{/if}
		</div>
	{:else}
		{#if codeMirrorPending}
			<p class="empty-state">Preparing split editor.</p>
		{:else if codeMirrorLoadError}
			<p class="empty-state">Split editor unavailable: {codeMirrorLoadError}</p>
		{/if}
		<div class="diff-host" bind:this={container}></div>
	{/if}
</div>

<style>
	.diff-shell {
		display: grid;
		gap: 0;
	}

	.diff-labels {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-bottom: 0;
		border-radius: 8px 8px 0 0;
		overflow: hidden;
		background: var(--fw-border);
	}

	.diff-labels span {
		min-width: 0;
		padding: 0.48rem 0.68rem;
		background: #f7f6f3;
		color: #596775;
		font-size: 0.78rem;
		font-weight: 800;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.diff-host,
	.assembly-diff-host {
		min-height: 72vh;
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 0 0 8px 8px;
		background: #fff;
	}

	.diff-host {
		overflow: hidden;
	}

	.assembly-diff-host {
		max-height: 72vh;
		overflow: auto;
	}

	.assembly-summary {
		position: sticky;
		top: 0;
		z-index: 2;
		padding: 0.42rem 0.7rem;
		border-bottom: 1px solid rgba(23, 32, 42, 0.08);
		background: #fff;
		color: #596775;
		font-size: 0.76rem;
		font-weight: 700;
	}

	.assembly-grid {
		display: grid;
		min-width: 1080px;
	}

	.assembly-row {
		display: grid;
		grid-template-columns: minmax(540px, 1fr) minmax(540px, 1fr);
		border-bottom: 1px solid rgba(23, 32, 42, 0.06);
	}

	.assembly-row:last-child {
		border-bottom: 0;
	}

	.assembly-cell {
		display: grid;
		grid-template-columns: 5.1rem max-content;
		align-items: stretch;
		min-width: 0;
		background: #fff;
	}

	.assembly-cell + .assembly-cell {
		border-left: 1px solid rgba(23, 32, 42, 0.08);
	}

	.asm-line-no,
	.asm-text {
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
		font-size: 0.78rem;
		line-height: 1.45;
		white-space: pre;
	}

	.asm-line-no {
		padding: 0 0.58rem;
		border-right: 1px solid rgba(23, 32, 42, 0.08);
		background: #f7f6f3;
		color: #7a8793;
		text-align: right;
		user-select: none;
	}

	.asm-text {
		padding: 0 0.7rem;
		color: #2c3742;
	}

	.assembly-row.changed .before-cell,
	.assembly-row.removed .before-cell {
		background: rgba(175, 35, 20, 0.06);
	}

	.assembly-row.changed .after-cell,
	.assembly-row.added .after-cell {
		background: rgba(61, 140, 99, 0.08);
	}

	.assembly-row.added .before-cell .asm-text,
	.assembly-row.added .before-cell .asm-line-no,
	.assembly-row.removed .after-cell .asm-text,
	.assembly-row.removed .after-cell .asm-line-no {
		color: transparent;
	}

	.assembly-gap-row {
		display: flex;
		align-items: center;
		min-width: 1080px;
		padding: 0.28rem 5.1rem;
		border-bottom: 1px solid rgba(23, 32, 42, 0.06);
		background: #f7f6f3;
		color: #6f7d8a;
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
		font-size: 0.76rem;
	}

	.empty-state {
		margin: 0;
		padding: 1rem;
		color: #697786;
		font-size: 0.86rem;
		font-weight: 600;
	}

	:global(.cm-mergeView) {
		height: 72vh;
	}

	:global(.cm-mergeViewEditors) {
		height: 100%;
	}

	:global(.cm-mergeViewEditor) {
		height: 100%;
	}

	:global(.cm-editor) {
		height: 100%;
	}

	:global(.cm-scroller) {
		overflow: auto;
	}

	:global(.cm-changedLine) {
		background: rgba(189, 74, 23, 0.08);
	}

	:global(.cm-deletedChunk) {
		background: rgba(175, 35, 20, 0.08);
	}

	:global(.cm-insertedChunk) {
		background: rgba(61, 140, 99, 0.1);
	}

	@media (max-width: 860px) {
		.diff-labels,
		.assembly-row {
			grid-template-columns: 1fr;
		}

		.assembly-cell + .assembly-cell {
			border-top: 1px solid rgba(23, 32, 42, 0.08);
			border-left: 0;
		}

		:global(.cm-mergeViewEditors) {
			flex-direction: column;
		}
	}
</style>
