<script lang="ts">
	import type { DocsStatus } from '$lib/docs/navigation';

	let {
		steps
	}: {
		steps: readonly { label: string; detail: string; status?: DocsStatus }[];
	} = $props();
</script>

<ol class="pipeline-flow" aria-label="Processing pipeline">
	{#each steps as step, index (step.label)}
		<li>
			<div class="step-number">{String(index + 1).padStart(2, '0')}</div>
			<div class="step-copy">
				<header>
					<strong>{step.label}</strong>
					{#if step.status}<span data-status={step.status}>{step.status}</span>{/if}
				</header>
				<p>{step.detail}</p>
			</div>
		</li>
	{/each}
</ol>

<style>
	.pipeline-flow {
		margin: 1.5rem 0 0;
		padding: 0;
		border-top: 1px solid var(--watch-rule-strong);
		list-style: none;
	}

	.pipeline-flow li {
		display: grid;
		grid-template-columns: 3.25rem minmax(0, 1fr);
		gap: 1rem;
		padding: 1.1rem 0;
		border-bottom: 1px solid var(--watch-rule);
	}

	.step-number {
		padding-top: 0.08rem;
		color: var(--watch-blue-soft);
		font: 500 0.72rem/1.5 'Fragment Mono', monospace;
	}

	.step-copy header {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		justify-content: space-between;
		gap: 0.4rem 1rem;
	}

	.step-copy strong {
		font: 500 1.05rem/1.4 'Archivo SemiCondensed', sans-serif;
	}

	.step-copy span {
		color: var(--watch-muted);
		font: 500 0.68rem/1.4 'Fragment Mono', monospace;
	}

	.step-copy span[data-status='Implemented'] {
		color: #73b997;
	}

	.step-copy span[data-status='In progress'] {
		color: #d3ad62;
	}

	.step-copy span[data-status='Planned'] {
		color: #8ea8ee;
	}

	.step-copy p {
		max-width: 46rem;
		margin: 0.35rem 0 0;
		color: var(--watch-muted);
		font-size: 0.92rem;
		line-height: 1.55;
	}

	@media (max-width: 560px) {
		.pipeline-flow li {
			grid-template-columns: 2.2rem minmax(0, 1fr);
			gap: 0.65rem;
		}

		.step-copy header {
			display: block;
		}

		.step-copy span {
			display: block;
			margin-top: 0.25rem;
		}
	}
</style>
