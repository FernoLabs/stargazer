<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';

	type SubmitState = 'idle' | 'sending' | 'sent' | 'error';

	let name = $state('');
	let company = $state('');
	let telegram = $state('');
	let email = $state('');
	let message = $state('');
	let website = $state('');
	let submitState = $state<SubmitState>('idle');
	let errorMessage = $state('');

	const canSubmit = $derived(
		name.trim().length > 0 &&
			message.trim().length > 0 &&
			(telegram.trim().length > 0 || email.trim().length > 0) &&
			submitState !== 'sending'
	);

	function contactSource() {
		const intent = page.url.searchParams.get('intent');
		const plan = page.url.searchParams.get('plan');
		if (intent) return `intent:${intent}`;
		if (plan) return `plan:${plan}`;
		return page.url.pathname;
	}

	async function submitContact(event: SubmitEvent) {
		event.preventDefault();
		if (!canSubmit) return;

		submitState = 'sending';
		errorMessage = '';

		try {
			const response = await fetch(resolve('/api/contact'), {
				method: 'POST',
				headers: {
					'content-type': 'application/json'
				},
				body: JSON.stringify({
					name,
					company,
					telegram,
					email,
					message,
					website,
					source: contactSource()
				})
			});
			const payload = (await response.json()) as { ok?: boolean; error?: string };

			if (!response.ok || !payload.ok) {
				throw new Error(payload.error || 'Could not send the request.');
			}

			submitState = 'sent';
			name = '';
			company = '';
			telegram = '';
			email = '';
			message = '';
			website = '';
		} catch (error) {
			submitState = 'error';
			errorMessage = error instanceof Error ? error.message : 'Could not send the request.';
		}
	}
</script>

<svelte:head>
	<title>Contact FireWatch</title>
</svelte:head>

<div class="page-shell">
	<section class="contact-panel" aria-labelledby="contact-title">
		<div class="contact-copy">
			<h1 id="contact-title">Contact FireWatch</h1>
			<p>
				Tell us what you need monitored and where alerts should land. The request goes to the
				FireWatch team so we can route the right setup.
			</p>

			<div class="request-types" aria-label="Common FireWatch requests">
				<div>
					<strong>Custom watch</strong>
					<span>Programs, wallets, multisigs, accounts, and dependency alerts.</span>
				</div>
				<div>
					<strong>Protocol page</strong>
					<span>Private or public surface with triage and escalation workflows.</span>
				</div>
				<div>
					<strong>Delivery routing</strong>
					<span>Telegram now, plus PagerDuty, Slack, email, or ticketing by request.</span>
				</div>
			</div>
		</div>

		<form class="contact-form" onsubmit={submitContact}>
			<label>
				<span>Name</span>
				<input autocomplete="name" bind:value={name} maxlength="120" name="name" required />
			</label>

			<label>
				<span>Company or team</span>
				<input autocomplete="organization" bind:value={company} maxlength="140" name="company" />
			</label>

			<div class="field-grid">
				<label>
					<span>Telegram</span>
					<input
						autocomplete="username"
						bind:value={telegram}
						maxlength="80"
						name="telegram"
						placeholder="@handle"
					/>
				</label>

				<label>
					<span>Email</span>
					<input autocomplete="email" bind:value={email} maxlength="160" name="email" type="email" />
				</label>
			</div>

			<label>
				<span>What should we monitor?</span>
				<textarea
					bind:value={message}
					maxlength="2000"
					name="message"
					placeholder="Protocols, program addresses, wallets, multisigs, delivery channel, urgency..."
					required
					rows="6"
				></textarea>
			</label>

			<label aria-hidden="true" class="honeypot">
				<span>Website</span>
				<input autocomplete="off" bind:value={website} name="website" tabindex="-1" />
			</label>

			<div class="form-footer">
				<button disabled={!canSubmit} type="submit">
					{submitState === 'sending' ? 'Sending...' : 'Send request'}
				</button>
				<p class="form-note">Add Telegram or email so we can reply.</p>
			</div>

			{#if submitState === 'sent'}
				<p class="status success" role="status">Sent. We will follow up shortly.</p>
			{:else if submitState === 'error'}
				<p class="status error" role="alert">{errorMessage}</p>
			{/if}
		</form>
	</section>
</div>

<style>
	.page-shell {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem 0 2.4rem;
	}

	.contact-panel {
		display: grid;
		grid-template-columns: minmax(0, 0.85fr) minmax(320px, 1fr);
		gap: 1rem;
		align-items: start;
		background: var(--fw-panel);
		border: 1px solid var(--fw-border);
		border-radius: 10px;
		padding: 1rem;
	}

	.contact-copy,
	.contact-form {
		min-width: 0;
	}

	.contact-copy {
		display: grid;
		gap: 0.78rem;
		padding: 0.2rem 0.2rem 0.2rem 0;
	}

	h1,
	p {
		margin: 0;
	}

	h1 {
		font-size: 2rem;
		line-height: 1.05;
		letter-spacing: 0;
	}

	p {
		color: var(--fw-muted);
		line-height: 1.55;
	}

	.contact-copy p {
		max-width: 46rem;
		font-size: 0.94rem;
	}

	.request-types {
		display: grid;
		border-top: 1px solid rgba(23, 32, 42, 0.08);
	}

	.request-types div {
		display: grid;
		gap: 0.12rem;
		padding: 0.74rem 0;
		border-bottom: 1px solid rgba(23, 32, 42, 0.08);
	}

	.request-types strong {
		color: #17202a;
		font-size: 0.9rem;
		line-height: 1.2;
	}

	.request-types span {
		color: #5f6c79;
		font-size: 0.84rem;
		line-height: 1.45;
	}

	.contact-form {
		display: grid;
		gap: 0.72rem;
		padding: 0.92rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		background: #fbfaf8;
	}

	label {
		display: grid;
		gap: 0.28rem;
		min-width: 0;
	}

	label span,
	.form-note,
	.status {
		font-size: 0.78rem;
	}

	label span {
		color: #47525d;
		font-weight: 720;
		line-height: 1.2;
	}

	input,
	textarea {
		width: 100%;
		min-width: 0;
		border: 1px solid rgba(23, 32, 42, 0.16);
		border-radius: 7px;
		background: #fff;
		color: #17202a;
		font: inherit;
		font-size: 0.88rem;
		line-height: 1.4;
	}

	input {
		min-height: 2.28rem;
		padding: 0 0.7rem;
	}

	textarea {
		resize: vertical;
		min-height: 8rem;
		padding: 0.62rem 0.7rem;
	}

	input:focus,
	textarea:focus {
		border-color: rgba(173, 78, 34, 0.48);
		outline: 3px solid rgba(173, 78, 34, 0.14);
		outline-offset: 0;
	}

	.field-grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 0.64rem;
	}

	.honeypot {
		position: absolute;
		left: -100vw;
		width: 1px;
		height: 1px;
		overflow: hidden;
	}

	.form-footer {
		display: flex;
		flex-wrap: wrap;
		gap: 0.52rem;
		align-items: center;
		justify-content: space-between;
		padding-top: 0.18rem;
	}

	button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 2.3rem;
		padding: 0 0.86rem;
		border: 1px solid var(--fw-accent);
		border-radius: 7px;
		background: var(--fw-accent);
		color: #fff;
		font: inherit;
		font-size: 0.84rem;
		font-weight: 780;
		cursor: pointer;
	}

	button:hover:not(:disabled) {
		background: var(--fw-accent-strong);
		border-color: var(--fw-accent-strong);
	}

	button:disabled {
		opacity: 0.55;
		cursor: not-allowed;
	}

	.form-note {
		color: #62707c;
		font-weight: 650;
	}

	.status {
		padding-top: 0.12rem;
		font-weight: 700;
	}

	.success {
		color: #167044;
	}

	.error {
		color: #9c3929;
	}

	@media (max-width: 820px) {
		.contact-panel {
			grid-template-columns: 1fr;
		}

		.contact-copy {
			padding-right: 0;
		}
	}

	@media (max-width: 640px) {
		.page-shell {
			width: auto;
			margin: 0 0.5rem;
			padding-top: 0.8rem;
		}

		.contact-panel {
			padding: 0.86rem;
		}

		.field-grid {
			grid-template-columns: 1fr;
		}

		.form-footer {
			align-items: stretch;
		}

		button {
			width: 100%;
		}

		h1 {
			font-size: 1.8rem;
		}
	}
</style>
