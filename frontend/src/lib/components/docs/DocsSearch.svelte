<script lang="ts">
	import { resolve } from '$app/paths';
	import { docsItems, type DocsHref } from '$lib/docs/navigation';

	type SearchEntry = {
		key: string;
		href: DocsHref;
		hash?: string;
		label: string;
		page: string;
		description: string;
		searchText: string;
		kind: 'Page' | 'Section';
	};

	const searchIndex: readonly SearchEntry[] = docsItems.flatMap((item) => [
		{
			key: item.href,
			href: item.href,
			label: item.label,
			page: item.label,
			description: item.description,
			searchText: [item.label, item.description, ...item.keywords].join(' ').toLowerCase(),
			kind: 'Page' as const
		},
		...item.sections.map((section) => ({
			key: `${item.href}#${section.id}`,
			href: item.href,
			hash: section.id,
			label: section.label,
			page: item.label,
			description: item.description,
			searchText: [section.label, item.label].join(' ').toLowerCase(),
			kind: 'Section' as const
		}))
	]);

	let dialog: HTMLDialogElement | undefined;
	let input: HTMLInputElement | undefined;
	let query = $state('');

	let results = $derived.by(() => {
		const normalized = query.trim().toLowerCase();
		if (!normalized) return searchIndex.filter((entry) => entry.kind === 'Page');

		const tokens = normalized.split(/\s+/).filter(Boolean);
		return searchIndex
			.filter((entry) => tokens.every((token) => entry.searchText.includes(token)))
			.toSorted((left, right) => {
				const leftLabel = left.label.toLowerCase();
				const rightLabel = right.label.toLowerCase();
				const leftScore = leftLabel === normalized ? 0 : leftLabel.startsWith(normalized) ? 1 : 2;
				const rightScore = rightLabel === normalized ? 0 : rightLabel.startsWith(normalized) ? 1 : 2;
				return leftScore - rightScore || left.label.localeCompare(right.label);
			})
			.slice(0, 10);
	});

	function openSearch(): void {
		if (dialog?.open) {
			input?.focus();
			return;
		}
		query = '';
		dialog?.showModal();
		requestAnimationFrame(() => input?.focus());
	}

	function closeSearch(): void {
		dialog?.close();
	}

	function handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Escape' && dialog?.open) {
			event.preventDefault();
			closeSearch();
			return;
		}

		const target = event.target;
		if (
			target instanceof HTMLElement &&
			(target.matches('input, textarea, select') || target.isContentEditable)
		) {
			return;
		}

		if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
			event.preventDefault();
			openSearch();
		}

		if (!event.metaKey && !event.ctrlKey && !event.altKey && event.key === '/') {
			event.preventDefault();
			openSearch();
		}
	}

	function closeFromBackdrop(event: MouseEvent): void {
		if (event.target === dialog) closeSearch();
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<button class="docs-search-trigger" type="button" onclick={openSearch} aria-haspopup="dialog">
	<svg viewBox="0 0 20 20" aria-hidden="true">
		<circle cx="8.5" cy="8.5" r="5.5"></circle>
		<path d="m12.5 12.5 4 4"></path>
	</svg>
	<span>Search documentation</span>
	<kbd>⌘ K</kbd>
</button>

<dialog bind:this={dialog} class="docs-search-dialog" aria-labelledby="docs-search-title" onclick={closeFromBackdrop}>
	<div class="docs-search-panel">
		<header>
			<svg viewBox="0 0 20 20" aria-hidden="true">
				<circle cx="8.5" cy="8.5" r="5.5"></circle>
				<path d="m12.5 12.5 4 4"></path>
			</svg>
			<label id="docs-search-title" for="docs-search-input">Search documentation</label>
			<input
				bind:this={input}
				bind:value={query}
				id="docs-search-input"
				type="search"
				placeholder="Search architecture, backfill, alerts..."
				autocomplete="off"
			/>
			<button type="button" onclick={closeSearch} aria-label="Close search">Esc</button>
		</header>

		<div class="docs-search-results" aria-live="polite">
			{#if results.length}
				<p>{query.trim() ? `${results.length} matching results` : 'Documentation pages'}</p>
				<ul>
					{#each results as result (result.key)}
						<li>
							<a
								href={resolve(
									(result.hash ? `${result.href}#${result.hash}` : result.href) as
										| DocsHref
										| `${DocsHref}#${string}`
								)}
								onclick={closeSearch}
							>
								<span>
									<strong>{result.label}</strong>
									{#if result.kind === 'Section'}<small>{result.page}</small>{/if}
								</span>
								<p>{result.description}</p>
								<svg viewBox="0 0 20 20" aria-hidden="true"><path d="m7 4 6 6-6 6"></path></svg>
							</a>
						</li>
					{/each}
				</ul>
			{:else}
				<div class="docs-search-empty">
					<strong>No matching page</strong>
					<p>Try a protocol, component, or operation name.</p>
				</div>
			{/if}
		</div>
	</div>
</dialog>

<style>
	.docs-search-trigger {
		display: flex;
		align-items: center;
		gap: 0.65rem;
		width: min(28rem, 100%);
		height: 2.5rem;
		padding: 0 0.75rem;
		border: 1px solid var(--docs-border);
		border-radius: 0.45rem;
		background: var(--docs-surface);
		color: var(--docs-muted);
		font: 400 0.84rem/1 'Source Sans 3', sans-serif;
		text-align: left;
		cursor: pointer;
		transition: border-color 140ms ease, color 140ms ease;
	}

	.docs-search-trigger:hover {
		border-color: var(--docs-border-strong);
		color: var(--docs-ink);
	}

	.docs-search-trigger:focus-visible,
	.docs-search-panel header button:focus-visible {
		outline: 2px solid var(--docs-accent);
		outline-offset: 2px;
	}

	.docs-search-trigger svg,
	.docs-search-panel header > svg {
		width: 1rem;
		height: 1rem;
		fill: none;
		stroke: currentColor;
		stroke-linecap: round;
		stroke-width: 1.6;
	}

	.docs-search-trigger span {
		flex: 1;
	}

	kbd {
		padding: 0.2rem 0.35rem;
		border: 1px solid var(--docs-border);
		border-radius: 0.25rem;
		background: var(--docs-bg);
		color: var(--docs-muted);
		font: 400 0.68rem/1 'Fragment Mono', monospace;
	}

	.docs-search-dialog {
		width: min(42rem, calc(100vw - 2rem));
		max-height: min(38rem, calc(100dvh - 4rem));
		padding: 0;
		border: 1px solid var(--docs-border-strong);
		border-radius: 0.6rem;
		background: var(--docs-surface);
		color: var(--docs-ink);
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
	}

	.docs-search-dialog::backdrop {
		background: rgba(0, 0, 0, 0.68);
	}

	.docs-search-panel {
		display: flex;
		max-height: min(38rem, calc(100dvh - 4rem));
		flex-direction: column;
	}

	.docs-search-panel > header {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.75rem;
		padding: 0.75rem 0.8rem;
		border-bottom: 1px solid var(--docs-border);
		color: var(--docs-muted);
	}

	.docs-search-panel label {
		position: absolute;
		width: 1px;
		height: 1px;
		padding: 0;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border: 0;
	}

	.docs-search-panel input {
		width: 100%;
		padding: 0;
		border: 0;
		outline: 0;
		background: transparent;
		color: var(--docs-ink);
		font: 400 1rem/1.5 'Source Sans 3', sans-serif;
	}

	.docs-search-panel input:focus-visible {
		outline: 0;
	}

	.docs-search-panel > header:focus-within {
		box-shadow: inset 0 -2px 0 var(--docs-accent);
	}

	.docs-search-panel input::placeholder {
		color: var(--docs-muted);
	}

	.docs-search-panel header button {
		padding: 0.25rem 0.4rem;
		border: 1px solid var(--docs-border);
		border-radius: 0.25rem;
		background: transparent;
		color: var(--docs-muted);
		font: 400 0.68rem/1 'Fragment Mono', monospace;
		cursor: pointer;
	}

	.docs-search-results {
		overflow-y: auto;
		padding: 0.75rem;
	}

	.docs-search-results > p {
		margin: 0 0 0.5rem;
		padding: 0 0.35rem;
		color: var(--docs-muted);
		font-size: 0.75rem;
	}

	.docs-search-results ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.docs-search-results li + li {
		margin-top: 0.2rem;
	}

	.docs-search-results a {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 0.2rem 1rem;
		padding: 0.7rem 0.75rem;
		border-radius: 0.4rem;
		color: var(--docs-ink);
		text-decoration: none;
	}

	.docs-search-results a:hover,
	.docs-search-results a:focus-visible {
		background: var(--docs-surface-raised);
		outline: 2px solid var(--docs-accent);
		outline-offset: -2px;
	}

	.docs-search-results a > span {
		display: flex;
		align-items: baseline;
		gap: 0.65rem;
	}

	.docs-search-results strong {
		font-size: 0.9rem;
		font-weight: 600;
	}

	.docs-search-results small {
		color: var(--docs-muted);
		font-size: 0.72rem;
	}

	.docs-search-results a > p {
		grid-column: 1;
		margin: 0;
		color: var(--docs-muted);
		font-size: 0.78rem;
		line-height: 1.35;
	}

	.docs-search-results a > svg {
		grid-column: 2;
		grid-row: 1 / span 2;
		align-self: center;
		width: 0.9rem;
		height: 0.9rem;
		fill: none;
		stroke: var(--docs-muted);
		stroke-linecap: round;
		stroke-linejoin: round;
		stroke-width: 1.5;
	}

	.docs-search-empty {
		padding: 3rem 1rem;
		text-align: center;
	}

	.docs-search-empty strong {
		font-weight: 600;
	}

	.docs-search-empty p {
		margin: 0.35rem 0 0;
		color: var(--docs-muted);
		font-size: 0.85rem;
	}

	@media (max-width: 720px) {
		.docs-search-trigger {
			width: 2.5rem;
			justify-content: center;
			padding: 0;
		}

		.docs-search-trigger span,
		.docs-search-trigger kbd {
			display: none;
		}

		.docs-search-dialog {
			width: calc(100vw - 1rem);
			max-height: calc(100dvh - 1rem);
		}

		.docs-search-panel {
			max-height: calc(100dvh - 1rem);
		}
	}
</style>
