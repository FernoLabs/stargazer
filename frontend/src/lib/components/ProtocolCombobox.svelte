<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { protocolSlugAliases } from '$lib/catalog';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import type { DefiProtocolDescriptor } from '$lib/types';

	type ProtocolOption = {
		slug: string;
		name: string;
		aliases: string[];
	};

	let {
		protocols,
		value,
		onSelect,
		label = 'Protocol'
	}: {
		protocols: DefiProtocolDescriptor[];
		value: string;
		onSelect: (slug: string) => void;
		label?: string;
	} = $props();

	const listboxId = 'protocol-combobox-listbox';
	let root = $state<HTMLDivElement | null>(null);
	let trigger = $state<HTMLButtonElement | null>(null);
	let searchInput = $state<HTMLInputElement | null>(null);
	let open = $state(false);
	let query = $state('');
	let activeIndex = $state(0);

	const options = $derived.by<ProtocolOption[]>(() => [
		{ slug: 'all', name: 'All protocols', aliases: ['all'] },
		...protocols.map((protocol) => ({
			slug: protocol.slug,
			name: protocol.name,
			aliases: [
				protocol.slug,
				...protocolSlugAliases(protocol.slug),
				...protocolSlugAliases(protocol.slug).map((slug) => slug.replaceAll('_', ' '))
			]
		}))
	]);
	const selectedOption = $derived(
		options.find((option) => option.slug === value) ?? options[0]
	);
	const filteredOptions = $derived.by(() => {
		const needle = query.trim().toLowerCase();
		if (!needle) return options;
		return options.filter((option) =>
			[option.name, option.slug, ...option.aliases].some((item) =>
				item.toLowerCase().includes(needle)
			)
		);
	});
	const clampedActiveIndex = $derived(
		Math.min(activeIndex, Math.max(0, filteredOptions.length - 1))
	);
	const activeOption = $derived(filteredOptions[clampedActiveIndex] ?? null);
	const activeDescendant = $derived(open && activeOption ? optionId(activeOption.slug) : undefined);

	function optionId(slug: string) {
		return `${listboxId}-${slug.replace(/[^a-z0-9_-]/gi, '-')}`;
	}

	async function openList() {
		open = true;
		query = '';
		activeIndex = Math.max(
			0,
			options.findIndex((option) => option.slug === value)
		);
		await tick();
		searchInput?.focus();
	}

	function closeList() {
		open = false;
		query = '';
	}

	function toggleList() {
		if (open) {
			closeList();
			return;
		}
		void openList();
	}

	function selectOption(slug: string) {
		onSelect(slug);
		closeList();
		trigger?.focus();
	}

	function updateQuery(event: Event) {
		query = (event.currentTarget as HTMLInputElement).value;
		activeIndex = 0;
	}

	function moveActive(delta: number) {
		if (!filteredOptions.length) return;
		activeIndex = (activeIndex + delta + filteredOptions.length) % filteredOptions.length;
	}

	function handleTriggerKeydown(event: KeyboardEvent) {
		if (event.key === 'ArrowDown' || event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			void openList();
		}
		if (event.key === 'ArrowUp') {
			event.preventDefault();
			void openList().then(() => {
				activeIndex = Math.max(0, filteredOptions.length - 1);
			});
		}
	}

	function handleSearchKeydown(event: KeyboardEvent) {
		if (event.key === 'ArrowDown') {
			event.preventDefault();
			moveActive(1);
		}
		if (event.key === 'ArrowUp') {
			event.preventDefault();
			moveActive(-1);
		}
		if (event.key === 'Home') {
			activeIndex = 0;
		}
		if (event.key === 'End') {
			activeIndex = Math.max(0, filteredOptions.length - 1);
		}
		if (event.key === 'Enter' && activeOption) {
			event.preventDefault();
			selectOption(activeOption.slug);
		}
		if (event.key === 'Escape') {
			event.preventDefault();
			closeList();
			trigger?.focus();
		}
	}

	onMount(() => {
		function handlePointerDown(event: PointerEvent) {
			if (open && !root?.contains(event.target as Node)) closeList();
		}

		window.addEventListener('pointerdown', handlePointerDown);
		return () => window.removeEventListener('pointerdown', handlePointerDown);
	});
</script>

<div class="protocol-combobox" bind:this={root}>
	<button
		bind:this={trigger}
		class="combo-trigger"
		type="button"
		role="combobox"
		aria-label={label}
		aria-expanded={open}
		aria-controls={listboxId}
		aria-haspopup="listbox"
		aria-activedescendant={activeDescendant}
		onclick={toggleList}
		onkeydown={handleTriggerKeydown}
	>
		{#if selectedOption.slug === 'all'}
			<span class="all-mark" aria-hidden="true"></span>
			<span class="trigger-label">{selectedOption.name}</span>
		{:else}
			<ProtocolIdentity slug={selectedOption.slug} label={selectedOption.name} size="sm" />
		{/if}
		<span class="chevron" aria-hidden="true"></span>
	</button>

	{#if open}
		<div class="combo-popover">
			<input
				bind:this={searchInput}
				class="combo-search"
				type="search"
				value={query}
				placeholder="Search protocol"
				aria-label="Search protocol"
				autocomplete="off"
				spellcheck="false"
				oninput={updateQuery}
				onkeydown={handleSearchKeydown}
			/>
			<div id={listboxId} class="combo-list" role="listbox" aria-label="Protocol options">
				{#if filteredOptions.length === 0}
					<p class="empty-option">No protocol found.</p>
				{:else}
					{#each filteredOptions as option, index (option.slug)}
						<button
							id={optionId(option.slug)}
							class:active-option={index === clampedActiveIndex}
							class:selected-option={option.slug === value}
							class="combo-option"
							type="button"
							role="option"
							aria-selected={option.slug === value}
							onclick={() => selectOption(option.slug)}
							onmousemove={() => (activeIndex = index)}
						>
							{#if option.slug === 'all'}
								<span class="all-mark" aria-hidden="true"></span>
								<span>All protocols</span>
							{:else}
								<ProtocolIdentity slug={option.slug} label={option.name} size="sm" />
							{/if}
						</button>
					{/each}
				{/if}
			</div>
		</div>
	{/if}
</div>

<style>
	.protocol-combobox {
		position: relative;
		min-width: 0;
	}

	.combo-trigger,
	.combo-search,
	.combo-option {
		font: inherit;
	}

	.combo-trigger {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.44rem;
		width: 100%;
		min-height: 2.34rem;
		padding: 0.48rem 0.62rem;
		border: 1px solid rgba(23, 32, 42, 0.12);
		border-radius: 8px;
		background: #fff;
		color: var(--fw-ink);
		cursor: pointer;
		text-align: left;
	}

	.combo-trigger:focus-visible,
	.combo-search:focus-visible {
		border-color: rgba(173, 78, 34, 0.48);
		box-shadow: 0 0 0 3px rgba(173, 78, 34, 0.1);
		outline: none;
	}

	.trigger-label,
	.combo-option > span:not(.all-mark) {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.chevron {
		width: 0.48rem;
		height: 0.48rem;
		border-right: 2px solid currentColor;
		border-bottom: 2px solid currentColor;
		opacity: 0.72;
		transform: translateY(-0.12rem) rotate(45deg);
	}

	.all-mark {
		display: inline-block;
		width: 1rem;
		height: 1rem;
		border: 1px solid rgba(23, 32, 42, 0.18);
		border-radius: 5px;
		background: linear-gradient(180deg, #fff, #f3f1ed);
	}

	.combo-trigger .all-mark,
	.combo-option .all-mark {
		margin-right: 0.38rem;
	}

	.combo-popover {
		position: absolute;
		top: calc(100% + 0.3rem);
		left: 0;
		z-index: 50;
		width: min(22rem, calc(100vw - 2rem));
		max-width: 100%;
		padding: 0.42rem;
		border: 1px solid rgba(23, 32, 42, 0.16);
		border-radius: 8px;
		background: #fff;
		box-shadow: 0 8px 22px rgba(23, 32, 42, 0.12);
	}

	.combo-search {
		width: 100%;
		min-height: 2.18rem;
		margin-bottom: 0.34rem;
		padding: 0.44rem 0.56rem;
		border: 1px solid rgba(23, 32, 42, 0.12);
		border-radius: 7px;
		background: #fff;
		color: var(--fw-ink);
		font-size: 0.84rem;
		box-sizing: border-box;
	}

	.combo-list {
		display: grid;
		gap: 0.08rem;
		max-height: 18rem;
		overflow-y: auto;
	}

	.combo-option {
		display: flex;
		align-items: center;
		width: 100%;
		min-width: 0;
		min-height: 2rem;
		padding: 0.36rem 0.44rem;
		border: 0;
		border-radius: 7px;
		background: transparent;
		color: var(--fw-ink);
		cursor: pointer;
		text-align: left;
	}

	.combo-option.active-option {
		background: rgba(173, 78, 34, 0.1);
	}

	.combo-option.selected-option {
		font-weight: 760;
	}

	.empty-option {
		margin: 0;
		padding: 0.58rem 0.44rem;
		color: var(--fw-muted);
		font-size: 0.82rem;
	}
</style>
