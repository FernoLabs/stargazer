<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { parseStellarWalletAddress } from '$lib/dashboard/stellar';
	import { untrack } from 'svelte';

	interface Props {
		label?: string;
		description?: string;
		initialValue?: string;
		compact?: boolean;
		submitLabel?: string;
	}

	let {
		label = 'Stellar wallet address',
		description = 'Enter a public G- or M-address. Stargazer never asks for a secret key.',
		initialValue = '',
		compact = false,
		submitLabel = 'Check coverage'
	}: Props = $props();

	let value = $state(untrack(() => initialValue));
	let error = $state<string | null>(null);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const parsed = parseStellarWalletAddress(value);
		if (!parsed.ok) {
			error = parsed.error.message;
			return;
		}

		error = null;
		await goto(resolve('/app/wallet/[address]', { address: parsed.value.address }));
	}
</script>

<form class:compact onsubmit={submit} novalidate>
	<label for="wallet-address">{label}</label>
	<div class="search-row">
		<input
			id="wallet-address"
			name="address"
			type="text"
			bind:value
			oninput={() => (error = null)}
			aria-describedby="wallet-address-hint wallet-address-error"
			aria-invalid={error ? 'true' : undefined}
			autocomplete="off"
			placeholder="G… or M…"
			spellcheck="false"
		/>
		<button type="submit">{submitLabel} <span aria-hidden="true">→</span></button>
	</div>
	<p id="wallet-address-hint" class="hint">{description}</p>
	<p id="wallet-address-error" class="error" aria-live="polite">{error ?? ''}</p>
</form>

<style>
	form {
		width: 100%;
	}

	label {
		display: block;
		margin-bottom: 0.55rem;
		font-size: 0.79rem;
		font-weight: 600;
		letter-spacing: 0.01em;
	}

	.search-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		border: 1px solid var(--app-rule-strong);
		border-radius: 6px;
		background: var(--app-paper);
		overflow: hidden;
		transition: border-color 120ms ease, box-shadow 120ms ease;
	}

	.search-row:focus-within {
		border-color: var(--app-violet);
		box-shadow: 0 0 0 2px var(--app-violet-soft);
	}

	input {
		min-width: 0;
		border: 0;
		background: transparent;
		color: var(--app-ink);
		font: 0.92rem/1.3 'Fragment Mono', monospace;
		padding: 0.95rem 1rem;
		outline: none;
	}

	input::placeholder {
		color: #99969d;
	}

	input[aria-invalid='true'] {
		background: #fff7f8;
	}

	button {
		border: 0;
		border-left: 1px solid var(--app-rule-strong);
		background: var(--app-ink);
		color: white;
		cursor: pointer;
		font: 600 0.82rem/1 'Source Sans 3', sans-serif;
		padding: 0 1.25rem;
		white-space: nowrap;
	}

	button:hover {
		background: var(--app-violet);
	}

	button:focus-visible {
		outline: 2px solid var(--app-violet);
		outline-offset: -4px;
	}

	button span {
		margin-left: 0.45rem;
	}

	.hint,
	.error {
		margin: 0;
		font-size: 0.75rem;
	}

	.hint {
		margin-top: 0.55rem;
		color: var(--app-muted);
	}

	.error {
		min-height: 1.15rem;
		margin-top: 0.2rem;
		color: var(--app-danger);
		font-weight: 600;
	}

	.compact label,
	.compact .hint {
		display: none;
	}

	.compact .error {
		margin-bottom: -1.35rem;
	}

	.compact input {
		padding-block: 0.7rem;
	}

	@media (max-width: 580px) {
		.search-row {
			grid-template-columns: 1fr;
			overflow: visible;
			border: 0;
			background: transparent;
			box-shadow: none;
			gap: 0.55rem;
		}

		.search-row:focus-within {
			box-shadow: none;
		}

		input {
			border: 1px solid var(--app-rule-strong);
			border-radius: 6px;
			background: var(--app-paper);
		}

		button {
			min-height: 2.75rem;
			border: 0;
			border-radius: 6px;
		}
	}
</style>
