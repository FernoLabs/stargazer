<script lang="ts">
	import { diffArrays, type ArrayChange } from 'diff';

	type AsmLine = {
		number: number | null;
		body: string;
		raw: string;
	};

	type DiffRow = {
		kind: 'context' | 'changed' | 'removed' | 'added' | 'gap';
		beforeNumber: number | null;
		afterNumber: number | null;
		beforeText: string;
		afterText: string;
		count?: number;
	};

	let {
		beforeText,
		afterText,
		ignoreRelocationNoise = true
	}: {
		beforeText: string;
		afterText: string;
		ignoreRelocationNoise?: boolean;
	} = $props();

	const contextLines = 3;

	function parseAsm(text: string) {
		return text
			.split('\n')
			.filter((line) => line.length > 0)
			.map((raw, index): AsmLine => {
				const match = /^(\d+):\s?(.*)$/.exec(raw);
				return {
					number: match ? Number(match[1]) : null,
					body: match?.[2] ?? raw,
					raw
				};
			});
	}

	function normalizeBody(body: string) {
		if (!ignoreRelocationNoise) return body;
		return body
			.replace(/\bcall\s+0x[0-9a-f]+\b/g, 'call <target>')
			.replace(/\bgoto\s+[+-]0x[0-9a-f]+\b/g, 'goto <offset>')
			.replace(/\b0x[0-9a-f]{5,}\s+ll\b/g, '<addr> ll');
	}

	function pairChanged(
		before: AsmLine[],
		after: AsmLine[],
		beforeIndex: number,
		afterIndex: number
	): DiffRow[] {
		const rows: DiffRow[] = [];
		const count = Math.max(before.length, after.length);
		for (let index = 0; index < count; index += 1) {
			const beforeLine = before[index];
			const afterLine = after[index];
			rows.push({
				kind: beforeLine && afterLine ? 'changed' : beforeLine ? 'removed' : 'added',
				beforeNumber: beforeLine ? beforeLine.number : beforeIndex + index,
				afterNumber: afterLine ? afterLine.number : afterIndex + index,
				beforeText: beforeLine?.body ?? '',
				afterText: afterLine?.body ?? ''
			});
		}
		return rows;
	}

	function allRows() {
		const before = parseAsm(beforeText);
		const after = parseAsm(afterText);
		const beforeBodies = before.map((line) => normalizeBody(line.body));
		const afterBodies = after.map((line) => normalizeBody(line.body));
		const changes = diffArrays(beforeBodies, afterBodies) as ArrayChange<string>[];
		const rows: DiffRow[] = [];
		let beforeIndex = 0;
		let afterIndex = 0;

		for (let index = 0; index < changes.length; index += 1) {
			const change = changes[index];
			const next = changes[index + 1];

			if (!change.added && !change.removed) {
				for (let offset = 0; offset < change.value.length; offset += 1) {
					const beforeLine = before[beforeIndex + offset];
					const afterLine = after[afterIndex + offset];
					rows.push({
						kind: 'context',
						beforeNumber: beforeLine ? beforeLine.number : beforeIndex + offset,
						afterNumber: afterLine ? afterLine.number : afterIndex + offset,
						beforeText: beforeLine?.body ?? '',
						afterText: afterLine?.body ?? ''
					});
				}
				beforeIndex += change.value.length;
				afterIndex += change.value.length;
				continue;
			}

			if (change.removed && next?.added) {
				const removed = before.slice(beforeIndex, beforeIndex + change.value.length);
				const added = after.slice(afterIndex, afterIndex + next.value.length);
				rows.push(...pairChanged(removed, added, beforeIndex, afterIndex));
				beforeIndex += change.value.length;
				afterIndex += next.value.length;
				index += 1;
				continue;
			}

			if (change.removed) {
				const removed = before.slice(beforeIndex, beforeIndex + change.value.length);
				rows.push(...pairChanged(removed, [], beforeIndex, afterIndex));
				beforeIndex += change.value.length;
				continue;
			}

			if (change.added) {
				const added = after.slice(afterIndex, afterIndex + change.value.length);
				rows.push(...pairChanged([], added, beforeIndex, afterIndex));
				afterIndex += change.value.length;
			}
		}

		return rows;
	}

	function visibleRows() {
		const rows = allRows();
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

		const output: DiffRow[] = [];
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

	function changedCount() {
		return allRows().filter((row) => row.kind !== 'context').length;
	}
</script>

<div class="changes-shell">
	<div class="changes-head">
		<span>Changed instruction rows</span>
		<strong>{changedCount()}</strong>
	</div>

	<div class="changes-table">
		<div class="table-head">
			<span>Before</span>
			<span></span>
			<span>After</span>
		</div>
		{#each visibleRows() as row}
			{#if row.kind === 'gap'}
				<div class="gap-row">
					<span>{row.count} unchanged instructions hidden</span>
				</div>
			{:else}
				<div class:context-row={row.kind === 'context'} class:changed-row={row.kind === 'changed'} class:removed-row={row.kind === 'removed'} class:added-row={row.kind === 'added'} class="change-row">
					<div>
						<code class="line-no">{row.beforeNumber === null ? '' : String(row.beforeNumber).padStart(6, '0')}</code>
						<code>{row.beforeText}</code>
					</div>
					<span class="arrow">→</span>
					<div>
						<code class="line-no">{row.afterNumber === null ? '' : String(row.afterNumber).padStart(6, '0')}</code>
						<code>{row.afterText}</code>
					</div>
				</div>
			{/if}
		{/each}
	</div>
</div>

<style>
	.changes-shell {
		display: grid;
		gap: 0.62rem;
	}

	.changes-head {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		color: #647280;
		font-size: 0.82rem;
		font-weight: 600;
	}

	.changes-head strong {
		color: #1c2731;
	}

	.changes-table {
		display: grid;
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 8px;
		overflow: hidden;
		background: #fff;
	}

	.table-head,
	.change-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 2rem minmax(0, 1fr);
		align-items: stretch;
	}

	.table-head {
		background: #f7f6f3;
		border-bottom: 1px solid rgba(23, 32, 42, 0.08);
		color: #6f7d8a;
		font-size: 0.78rem;
		font-weight: 700;
	}

	.table-head span {
		padding: 0.5rem 0.68rem;
	}

	.change-row {
		border-bottom: 1px solid rgba(23, 32, 42, 0.06);
	}

	.change-row:last-child {
		border-bottom: 0;
	}

	.change-row > div {
		display: grid;
		grid-template-columns: 4.9rem minmax(0, 1fr);
		gap: 0.5rem;
		min-width: 0;
		padding: 0.32rem 0.58rem;
	}

	.context-row {
		background: #fff;
	}

	.changed-row > div:first-child,
	.removed-row > div:first-child {
		background: rgba(175, 35, 20, 0.07);
	}

	.changed-row > div:last-child,
	.added-row > div:last-child {
		background: rgba(61, 140, 99, 0.09);
	}

	.arrow {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		color: #8a96a3;
		background: #faf9f6;
		border-left: 1px solid rgba(23, 32, 42, 0.06);
		border-right: 1px solid rgba(23, 32, 42, 0.06);
	}

	code {
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
		font-size: 0.78rem;
		color: #2c3742;
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	.line-no {
		color: #7a8793;
	}

	.gap-row {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0.38rem 0.58rem;
		background: #fbfaf8;
		color: #74818e;
		font-size: 0.78rem;
		font-weight: 600;
		border-bottom: 1px solid rgba(23, 32, 42, 0.06);
	}

	@media (max-width: 820px) {
		.table-head,
		.change-row {
			grid-template-columns: 1fr;
		}

		.table-head span:nth-child(2),
		.arrow {
			display: none;
		}
	}
</style>
