<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import DocsSearch from '$lib/components/docs/DocsSearch.svelte';
	import { docsItems, docsNavigation, getDocsGroup, type DocsHref } from '$lib/docs/navigation';
	import '$lib/styles/docs.css';
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();

	let activeIndex = $derived(Math.max(0, docsItems.findIndex((item) => item.href === page.url.pathname)));
	let activeItem = $derived(docsItems[activeIndex]);
	let activeGroup = $derived(getDocsGroup(activeItem));
	let previousItem = $derived(activeIndex > 0 ? docsItems[activeIndex - 1] : undefined);
	let nextItem = $derived(activeIndex < docsItems.length - 1 ? docsItems[activeIndex + 1] : undefined);
	let copiedPath = $state('');
	let copied = $derived(copiedPath === page.url.pathname);
	let mobileNavigation: HTMLDetailsElement | undefined;

	async function copyPageLink(): Promise<void> {
		await navigator.clipboard.writeText(page.url.href);
		copiedPath = page.url.pathname;
	}

	function closeMobileNavigation(): void {
		if (mobileNavigation) mobileNavigation.open = false;
	}
</script>

<div class="docs-shell" id="docs-top">
	<a class="docs-skip-link" href="#docs-content">Skip to documentation</a>

	<header class="docs-toolbar">
		<div class="docs-toolbar-inner">
			<a class="docs-home" href={resolve('/docs')}>
				<svg viewBox="0 0 24 24" aria-hidden="true">
					<path d="M12 1.5c0 6.1-2.8 9.1-9.1 9.8 6.3.7 9.1 3.7 9.1 9.8 0-6.1 2.8-9.1 9.1-9.8C14.8 10.6 12 7.6 12 1.5Z"></path>
				</svg>
				<span><strong>Stargazer</strong> Docs</span>
			</a>

			<DocsSearch />

			<nav class="docs-toolbar-actions" aria-label="Documentation actions">
				<a href={resolve('/app')}>Open demo</a>
				<a href={resolve('/docs/download')} download="stargazer-technical-documentation.pdf">PDF</a>
			</nav>
		</div>
	</header>

	<details bind:this={mobileNavigation} class="docs-mobile-navigation">
		<summary>
			<span>Browse documentation</span>
			<strong>{activeItem.label}</strong>
		</summary>
		<nav aria-label="Technical documentation">
			{#each docsNavigation as group (group.label)}
				<div>
					<strong>{group.label}</strong>
					<ul>
						{#each group.items as item (item.href)}
							<li>
								<a
									href={resolve(item.href)}
									aria-current={item.href === page.url.pathname ? 'page' : undefined}
									onclick={closeMobileNavigation}
								>
									{item.label}
								</a>
							</li>
						{/each}
					</ul>
				</div>
			{/each}
		</nav>
	</details>

	<div class="docs-grid">
		<aside class="docs-sidebar">
			<nav aria-label="Technical documentation">
				{#each docsNavigation as group (group.label)}
					<div class="docs-nav-group">
						<strong>{group.label}</strong>
						<ul>
							{#each group.items as item (item.href)}
								<li>
									<a
										href={resolve(item.href)}
										aria-current={item.href === page.url.pathname ? 'page' : undefined}
									>
										{item.label}
									</a>
								</li>
							{/each}
						</ul>
					</div>
				{/each}
			</nav>

			<div class="docs-sidebar-links">
				<a href={resolve('/app')}>Stellar demo <span aria-hidden="true">↗</span></a>
				<a href={resolve('/docs/download')} download="stargazer-technical-documentation.pdf">Download PDF <span aria-hidden="true">↓</span></a>
			</div>
		</aside>

		<main class="docs-content" id="docs-content" tabindex="-1">
			<nav class="docs-breadcrumbs" aria-label="Breadcrumb">
				<a href={resolve('/docs')}>Docs</a>
				<span aria-hidden="true">/</span>
				<span>{activeGroup.label}</span>
				<span aria-hidden="true">/</span>
				<strong>{activeItem.label}</strong>
			</nav>

			{@render children()}

			<nav class="docs-pagination" aria-label="Documentation pages">
				{#if previousItem}
					<a class="docs-previous" href={resolve(previousItem.href)}>
						<span>Previous</span>
						<strong>← {previousItem.label}</strong>
					</a>
				{:else}
					<span></span>
				{/if}
				{#if nextItem}
					<a class="docs-next" href={resolve(nextItem.href)}>
						<span>Next</span>
						<strong>{nextItem.label} →</strong>
					</a>
				{/if}
			</nav>
		</main>

		<aside class="docs-outline">
			<nav aria-label="On this page">
				<strong>On this page</strong>
				<ol>
					{#each activeItem.sections as section (section.id)}
						<li>
							<a
								href={resolve(`${activeItem.href}#${section.id}` as `${DocsHref}#${string}`)}
								aria-current={page.url.hash === `#${section.id}` ? 'location' : undefined}
							>
								{section.label}
							</a>
						</li>
					{/each}
				</ol>
				<div class="docs-outline-actions">
					<button type="button" onclick={copyPageLink}>
						<svg viewBox="0 0 20 20" aria-hidden="true">
							<rect x="6" y="6" width="10" height="10" rx="1"></rect>
							<path d="M4 13V4h9"></path>
						</svg>
						{copied ? 'Link copied' : 'Copy page link'}
					</button>
					<a href="#docs-top">Back to top ↑</a>
				</div>
			</nav>
		</aside>
	</div>
</div>
