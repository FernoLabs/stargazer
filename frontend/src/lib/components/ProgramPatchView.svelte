<script lang="ts">
	import { diffArrays, type ArrayChange } from 'diff';
	import { normalizeAssemblyRelocations } from '$lib/program-assembly';

	type PatchRow = {
		kind: 'context' | 'added' | 'removed' | 'gap' | 'file';
		beforeNumber: number | null;
		afterNumber: number | null;
		text: string;
	};

	type AsmLine = {
		number: number | null;
		body: string;
	};

	let {
		beforeText,
		afterText,
		beforeLabel,
		afterLabel,
		ignoreRelocationNoise = false
	}: {
		beforeText: string;
		afterText: string;
		beforeLabel: string;
		afterLabel: string;
		ignoreRelocationNoise?: boolean;
	} = $props();

	const contextLines = 4;

	function parseAsm(text: string) {
		return text
			.split('\n')
			.filter((line) => line.length > 0)
			.map((raw): AsmLine => {
				const match = /^(\d+):\s?(.*)$/.exec(raw);
				return {
					number: match ? Number(match[1]) : null,
					body: match?.[2] ?? raw
				};
			});
	}

	function displayBody(body: string) {
		return ignoreRelocationNoise ? normalizeAssemblyRelocations(body) : body;
	}

	function lineNumber(line: AsmLine | undefined) {
		return line?.number ?? null;
	}

	function allRows() {
		const before = parseAsm(beforeText);
		const after = parseAsm(afterText);
		const beforeBodies = before.map((line) => displayBody(line.body));
		const afterBodies = after.map((line) => displayBody(line.body));
		const changes = diffArrays(beforeBodies, afterBodies) as ArrayChange<string>[];
		const rows: PatchRow[] = [
			{ kind: 'file', beforeNumber: null, afterNumber: null, text: `--- previous.asm ${beforeLabel}` },
			{ kind: 'file', beforeNumber: null, afterNumber: null, text: `+++ current.asm ${afterLabel}` }
		];
		let beforeIndex = 0;
		let afterIndex = 0;

		for (const change of changes) {
			if (!change.added && !change.removed) {
				for (let offset = 0; offset < change.value.length; offset += 1) {
					const beforeLine = before[beforeIndex + offset];
					const afterLine = after[afterIndex + offset];
					rows.push({
						kind: 'context',
						beforeNumber: lineNumber(beforeLine),
						afterNumber: lineNumber(afterLine),
						text: displayBody(afterLine?.body ?? beforeLine?.body ?? '')
					});
				}
				beforeIndex += change.value.length;
				afterIndex += change.value.length;
				continue;
			}

			if (change.removed) {
				for (let offset = 0; offset < change.value.length; offset += 1) {
					const beforeLine = before[beforeIndex + offset];
					rows.push({
						kind: 'removed',
						beforeNumber: lineNumber(beforeLine),
						afterNumber: null,
						text: beforeLine ? displayBody(beforeLine.body) : ''
					});
				}
				beforeIndex += change.value.length;
				continue;
			}

			if (change.added) {
				for (let offset = 0; offset < change.value.length; offset += 1) {
					const afterLine = after[afterIndex + offset];
					rows.push({
						kind: 'added',
						beforeNumber: null,
						afterNumber: lineNumber(afterLine),
						text: afterLine ? displayBody(afterLine.body) : ''
					});
				}
				afterIndex += change.value.length;
			}
		}

		return rows;
	}

	function visibleRows(rows: PatchRow[]) {
		const changed = rows
			.map((row, index) => (row.kind === 'added' || row.kind === 'removed' ? index : -1))
			.filter((index) => index >= 0);
		if (changed.length === 0) return rows.filter((row) => row.kind === 'file');

		const keep = new Set<number>([0, 1]);
		for (const index of changed) {
			const start = Math.max(2, index - contextLines);
			const end = Math.min(rows.length - 1, index + contextLines);
			for (let current = start; current <= end; current += 1) {
				keep.add(current);
			}
		}

		const output: PatchRow[] = [];
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
					text: `@@ ${omitted} unchanged instructions hidden @@`
				});
				omitted = 0;
			}
			output.push(rows[index]);
		}

		return output;
	}

	let rows = $derived(allRows());
	let patchRows = $derived(visibleRows(rows));

	function lineKind(line: string) {
		if (line.startsWith('@@')) return 'hunk';
		if (line.startsWith('+++') || line.startsWith('---')) return 'file';
		if (line.startsWith('+')) return 'added';
		if (line.startsWith('-')) return 'removed';
		return 'context';
	}

	function rowClass(row: PatchRow) {
		if (row.kind === 'gap') return 'hunk';
		if (row.kind === 'added' || row.kind === 'removed' || row.kind === 'file') return row.kind;
		return lineKind(row.text);
	}

	function rowPrefix(row: PatchRow) {
		if (row.kind === 'added') return '+';
		if (row.kind === 'removed') return '-';
		return ' ';
	}

	function rowNumber(row: PatchRow) {
		const value = row.kind === 'added' ? row.afterNumber : row.beforeNumber;
		return value === null ? '' : String(value).padStart(6, '0');
	}
</script>

<div class="patch-shell">
	{#if patchRows.length <= 2}
		<p class="empty-state">No assembly text is available for this patch.</p>
	{:else}
		<pre aria-label="Program patch view">{#each patchRows as row}<code class={`line ${rowClass(row)}`}><span class="line-no">{rowNumber(row)}</span><span class="prefix">{row.kind === 'file' || row.kind === 'gap' ? ' ' : rowPrefix(row)}</span><span class="line-text">{row.text}</span></code>{/each}</pre>
	{/if}
</div>

<style>
	.patch-shell {
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 8px;
		background: #fff;
		overflow: hidden;
	}

	pre {
		display: grid;
		max-height: 72vh;
		margin: 0;
		overflow: auto;
		background: #fff;
	}

	code {
		display: grid;
		grid-template-columns: 4.6rem 1.4rem max-content;
		min-width: max-content;
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
		font-size: 0.78rem;
		line-height: 1.45;
		color: #2c3742;
		white-space: pre;
	}

	.line-no {
		position: sticky;
		left: 0;
		z-index: 1;
		padding: 0 0.58rem;
		border-right: 1px solid rgba(23, 32, 42, 0.08);
		background: #f7f6f3;
		color: #7a8793;
		text-align: right;
		user-select: none;
	}

	.prefix {
		position: sticky;
		left: 4.2rem;
		z-index: 1;
		padding: 0 0.28rem;
		background: inherit;
		color: #7a8793;
		text-align: center;
		user-select: none;
	}

	.line-text {
		padding: 0 0.7rem;
	}

	.line.hunk .line-text,
	.line.file .line-text {
		color: #5f4d3f;
		font-weight: 700;
	}

	.line.hunk {
		background: #f7f6f3;
	}

	.line.added {
		background: rgba(61, 140, 99, 0.1);
	}

	.line.removed {
		background: rgba(175, 35, 20, 0.08);
	}

	.empty-state {
		margin: 0;
		padding: 0.74rem 0.82rem;
		color: #5f6c79;
		font-size: 0.88rem;
		font-weight: 600;
	}
</style>
