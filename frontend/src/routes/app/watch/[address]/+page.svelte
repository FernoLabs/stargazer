<script lang="ts">
	import { resolve } from '$app/paths';
	import { formatAddress, formatDatasetPeriod, formatExactCount } from '$lib/dashboard/format';
	import { onMount } from 'svelte';
	import type { ActionData, PageData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();
	type PairingStatus = 'pendingTelegram' | 'paired' | 'expired';

	interface PairingStatusView {
		status: PairingStatus;
		policy: {
			state: string;
		};
		billing: {
			mode: string;
			paymentState: string;
			entitlementState: string;
		};
	}

	interface SubmittedPairing extends PairingStatusView {
		telegramUrl: string;
		expiresAt: string;
		statusUrl?: string;
	}

	let refreshedPairing = $state<PairingStatusView | null>(null);
	let statusUnavailable = $state(false);
	let submittedPairing = $derived(
		form?.success && form.pairing ? (form.pairing as SubmittedPairing) : null
	);
	let pairingStatus = $derived(refreshedPairing?.status ?? submittedPairing?.status ?? null);
	let pairingBilling = $derived(refreshedPairing?.billing ?? submittedPairing?.billing ?? null);
	let datasetPeriod = $derived(
		data.kind === 'ready'
			? formatDatasetPeriod(data.dashboard.source.indexedFrom, data.dashboard.source.asOf)
			: null
	);

	const expiryFormatter = new Intl.DateTimeFormat('en-GB', {
		day: '2-digit',
		month: 'short',
		year: 'numeric',
		hour: '2-digit',
		minute: '2-digit',
		hourCycle: 'h23',
		timeZone: 'UTC'
	});

	function parsePairingStatus(input: unknown): PairingStatusView | null {
		if (typeof input !== 'object' || input === null) return null;
		const value = input as Record<string, unknown>;
		if (!['pendingTelegram', 'paired', 'expired'].includes(String(value.status))) return null;
		if (typeof value.policy !== 'object' || value.policy === null) return null;
		if (typeof value.billing !== 'object' || value.billing === null) return null;
		const policy = value.policy as Record<string, unknown>;
		const billing = value.billing as Record<string, unknown>;
		if (typeof policy.state !== 'string') return null;
		if (
			typeof billing.mode !== 'string' ||
			typeof billing.paymentState !== 'string' ||
			typeof billing.entitlementState !== 'string'
		) {
			return null;
		}
		return {
			status: value.status as PairingStatus,
			policy: { state: policy.state },
			billing: {
				mode: billing.mode,
				paymentState: billing.paymentState,
				entitlementState: billing.entitlementState
			}
		};
	}

	onMount(() => {
		const statusUrl = submittedPairing?.statusUrl;
		if (!statusUrl || submittedPairing.status !== 'pendingTelegram') return;
		const pollingUrl: string = statusUrl;

		let stopped = false;
		let retryCount = 0;
		let timeout: ReturnType<typeof setTimeout> | undefined;

		async function refreshStatus() {
			try {
				const response = await fetch(pollingUrl, {
					headers: { accept: 'application/json' },
					cache: 'no-store'
				});
				if (!response.ok) throw new Error(`Pairing status returned ${response.status}`);
				const nextStatus = parsePairingStatus(await response.json());
				if (!nextStatus) throw new Error('Pairing status response was invalid');
				if (stopped) return;

				refreshedPairing = nextStatus;
				statusUnavailable = false;
				retryCount = 0;
				if (nextStatus.status === 'pendingTelegram') {
					timeout = setTimeout(refreshStatus, 2_000);
				}
			} catch {
				if (stopped) return;
				retryCount += 1;
				if (retryCount < 3) {
					timeout = setTimeout(refreshStatus, 4_000);
				} else {
					statusUnavailable = true;
				}
			}
		}

		timeout = setTimeout(refreshStatus, 2_000);
		return () => {
			stopped = true;
			if (timeout) clearTimeout(timeout);
		};
	});
</script>

<svelte:head>
	<title>Set up wallet alerts · Stargazer</title>
	<meta name="description" content="Choose Stargazer wallet alerts and connect Telegram." />
	<meta name="referrer" content="no-referrer" />
</svelte:head>

{#if data.kind === 'ready'}
	<a
		class="back-link"
		href={resolve('/app/wallet/[address]', { address: data.dashboard.query.inputAddress })}
	>
		<span aria-hidden="true">←</span> Back to report
	</a>
{:else}
	<a class="back-link" href={resolve('/app')}>
		<span aria-hidden="true">←</span> Back to wallet search
	</a>
{/if}

{#if data.kind === 'invalid'}
	<section class="message-state">
		<p class="eyebrow">Invalid wallet</p>
		<h1>This alert setup cannot be opened.</h1>
		<p>{data.message}</p>
	</section>
{:else if data.kind === 'unavailable'}
	<section class="message-state">
		<p class="eyebrow">Coverage unavailable</p>
		<h1>Reload the wallet report before setting up alerts.</h1>
		<p>{data.message}</p>
		<a class="primary-button" href={resolve('/app/wallet/[address]', { address: data.address })}>
			Retry wallet report
		</a>
	</section>
{:else}
	<nav class="progress" aria-label="Wallet alert setup progress">
		<span><i>✓</i> Wallet</span>
		<span><i>✓</i> Report</span>
		<span aria-current={pairingStatus === 'paired' ? undefined : 'step'}>
			<i>{pairingStatus === 'paired' ? '✓' : '3'}</i> Telegram
		</span>
		<span class="unavailable-step" aria-current={pairingStatus === 'paired' ? 'step' : undefined}>
			<i>4</i> Payment & delivery unavailable
		</span>
	</nav>

	<header class="setup-header">
		<div>
			<p class="eyebrow">Telegram configuration</p>
			<h1>Save alert preferences for this wallet.</h1>
			<p>
				Choose a preference, create a private Telegram link, and press Start in the bot. Pairing
				does not activate live delivery or collect payment.
			</p>
		</div>
		<dl>
			<div>
				<dt>Wallet</dt>
				<dd title={data.dashboard.query.accountAddress}>
					{formatAddress(data.dashboard.query.accountAddress, { start: 10, end: 8 })}
				</dd>
			</div>
			<div>
				<dt>Archive period</dt>
				<dd>{datasetPeriod?.range ?? 'Unavailable'}</dd>
			</div>
			<div>
				<dt>Ledger range</dt>
				<dd>
					{#if
						data.dashboard.source.indexedLedgerStart !== undefined &&
						data.dashboard.source.indexedLedgerEnd !== undefined}
						{formatExactCount(data.dashboard.source.indexedLedgerStart)}–{formatExactCount(
							data.dashboard.source.indexedLedgerEnd
						)}
					{:else}
						Unavailable
					{/if}
				</dd>
			</div>
		</dl>
	</header>

	{#if submittedPairing}
		<section class="telegram-ready" aria-labelledby="telegram-ready-title">
			<div>
				<p class="eyebrow">Telegram link ready</p>
				<h2 id="telegram-ready-title">Continue in Telegram.</h2>
				<p>
					This one-time link is valid for 15 minutes and expires on
					{expiryFormatter.format(new Date(submittedPairing.expiresAt))} UTC. Press Start in
					Telegram before then.
				</p>
			</div>
			<a
				href={submittedPairing.telegramUrl}
				target="_blank"
				rel="noreferrer"
			>
				Open Telegram <span aria-hidden="true">↗</span>
			</a>
		</section>

		<div class="pairing-status" role="status" aria-live="polite" aria-atomic="true">
			{#if statusUnavailable}
				<strong>Pairing status unavailable</strong>
				<span>Automatic checks stopped after three attempts. You can still use the link before it expires.</span>
			{:else if pairingStatus === 'paired'}
				<strong>Telegram paired</strong>
				<span>
					{pairingBilling?.mode === 'pilot'
						? 'Pilot entitlement recorded; no payment is required.'
						: 'Payment and entitlement are not configured.'}
					Live delivery is not connected.
				</span>
			{:else if pairingStatus === 'expired'}
				<strong>Telegram link expired</strong>
				<span>Create a new one-time link to try again. Live delivery is not connected.</span>
			{:else}
				<strong>Waiting for Telegram</strong>
				<span>Open the link and press Start. Live delivery is not connected.</span>
			{/if}
		</div>
	{/if}

	<div class="setup-grid">
		<form method="POST">
			<header>
				<span>03</span>
				<div>
					<h2>Notification preferences</h2>
					<p>Choose at least one group. This saves configuration; it does not start delivery.</p>
				</div>
			</header>

			<fieldset>
				<legend>Choose report severity</legend>
				<p class="fieldset-help">
					These groups mirror the archive alert preview on the wallet report.
				</p>
				<label>
					<input
						type="checkbox"
						name="criticalAlerts"
						checked={form?.criticalAlerts ?? true}
					/>
					<span>
						<strong>Critical security alerts <i>Recommended</i></strong>
						<small>
							Preference for verified incident-rule matches. Governance and controller changes are
							planned, not included.
						</small>
					</span>
				</label>

				<label>
					<input
						type="checkbox"
						name="nonCriticalAlerts"
						checked={form?.nonCriticalAlerts ?? true}
					/>
					<span>
						<strong>Non-critical report items</strong>
						<small>
							Preference for named protocol relationships and address-only coverage gaps. This does
							not create live change detection.
						</small>
					</span>
				</label>
			</fieldset>

			<p class="planned-note">
				<strong>Planned, not selectable:</strong> contract upgrades, governance, controller and
				multisig changes.
			</p>

			{#if form?.message}
				<p class="form-error" role="alert">{form.message}</p>
			{/if}

			<button type="submit">
				{form?.success ? 'Create a new one-time link' : 'Save preferences & create Telegram link'}
				<span aria-hidden="true">→</span>
			</button>
		</form>

		<aside>
			<section>
				<header>
					<span>PREVIEW</span>
					<div>
						<h2>Telegram message</h2>
						<p>Format preview only. The delivery worker is not connected.</p>
					</div>
				</header>
				<div class="message-preview" aria-label="Telegram message preview">
					<div><strong>STARGAZER</strong><span>Archive match</span></div>
					<p><b>Configured incident rule matched</b></p>
					<p>This wallet interacted with a contract included in a historical incident rule.</p>
					<small>Retrospective evidence: ledger, transaction and source links</small>
				</div>
			</section>

			<section class="billing">
				<header>
					<span>04</span>
					<div>
						<h2>Payment & activation</h2>
						{#if pairingBilling?.mode === 'pilot'}
							<p>Pilot entitlement can be recorded after pairing; delivery remains disconnected.</p>
						{:else if pairingBilling?.mode === 'unconfigured'}
							<p>Payment is not configured, so the saved policy cannot become active.</p>
						{:else}
							<p>No payment can occur on this page. The paid plan is not configured.</p>
						{/if}
					</div>
				</header>
				<div class="billing-state">
					{#if pairingBilling?.mode === 'pilot'}
						<strong class="pilot-state">Pilot entitlement</strong>
						<span>Recorded after pairing · live delivery not connected</span>
					{:else}
						<strong>Unavailable</strong>
						<span>Payment rail and live delivery are not connected.</span>
					{/if}
				</div>
			</section>
		</aside>
	</div>

	<footer class="safety-note">
		<strong>No placeholder checkout</strong>
		<p>
			Stargazer will not accept funds until a payment provider, price, recipient, settlement check
			and entitlement policy are configured.
		</p>
	</footer>
{/if}

<style>
	.back-link {
		display: inline-flex;
		gap: 0.45rem;
		align-items: center;
		margin-bottom: 1.5rem;
		color: var(--app-muted);
		font-size: 0.875rem;
		font-weight: 600;
		text-decoration: none;
	}

	.back-link:hover {
		color: var(--app-violet);
	}

	.progress {
		display: flex;
		gap: 1.5rem;
		margin-bottom: 2.5rem;
		padding-bottom: 0.85rem;
		border-bottom: 1px solid var(--app-rule);
	}

	.progress span {
		display: inline-flex;
		gap: 0.45rem;
		align-items: center;
		color: var(--app-muted);
		font-size: 0.875rem;
		font-weight: 600;
	}

	.progress span[aria-current='step'] {
		color: var(--app-ink);
	}

	.progress .unavailable-step {
		color: var(--app-warning);
	}

	.progress i {
		display: grid;
		width: 1.25rem;
		height: 1.25rem;
		place-items: center;
		border: 1px solid var(--app-rule-strong);
		border-radius: 50%;
		font: 0.6rem/1 'Fragment Mono', monospace;
		font-style: normal;
	}

	.progress span[aria-current='step'] i {
		border-color: var(--app-violet);
		background: var(--app-violet);
		color: white;
	}

	.eyebrow {
		margin: 0 0 0.75rem;
		color: var(--app-violet);
		font: 0.66rem/1.3 'Fragment Mono', monospace;
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}

	.setup-header {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 3rem;
		align-items: end;
		margin-bottom: 2rem;
	}

	.setup-header h1,
	.message-state h1 {
		max-width: 16ch;
		margin: 0;
		font: 500 clamp(2.25rem, 4vw, 3.8rem)/1 'Archivo SemiCondensed', sans-serif;
		letter-spacing: -0.04em;
	}

	.setup-header > div > p:last-child,
	.message-state > p {
		max-width: 45rem;
		margin: 0.9rem 0 0;
		color: var(--app-muted);
		line-height: 1.5;
		font-size: 0.95rem;
	}

	.setup-header dl {
		display: flex;
		margin: 0;
		border-top: 1px solid var(--app-ink);
		border-bottom: 1px solid var(--app-rule);
		background: var(--app-paper);
	}

	.setup-header dl div {
		min-width: 9rem;
		padding: 0.75rem 0.9rem;
		border-right: 1px solid var(--app-rule);
	}

	.setup-header dl div:last-child {
		border-right: 0;
	}

	dt {
		color: var(--app-muted);
		font-size: 0.75rem;
		text-transform: uppercase;
	}

	dd {
		margin: 0.22rem 0 0;
		font: 600 0.78rem/1.4 'Fragment Mono', monospace;
	}

	.telegram-ready {
		display: flex;
		justify-content: space-between;
		gap: 2rem;
		align-items: center;
		margin-bottom: 1.5rem;
		padding: 1.1rem 1.2rem;
		border: 1px solid #a9cbbd;
		background: #f0faf5;
	}

	.telegram-ready h2,
	.setup-grid h2 {
		margin: 0;
		font: 600 1.05rem/1.2 'Archivo SemiCondensed', sans-serif;
	}

	.telegram-ready p:last-child {
		max-width: 48rem;
		margin: 0.3rem 0 0;
		color: var(--app-muted);
		font-size: 0.875rem;
		line-height: 1.45;
	}

	.telegram-ready > a,
	.primary-button {
		flex: 0 0 auto;
		padding: 0.78rem 1rem;
		border-radius: 5px;
		background: var(--app-ink);
		color: white;
		font-size: 0.875rem;
		font-weight: 600;
		text-decoration: none;
	}

	.telegram-ready > a:hover,
	.primary-button:hover {
		background: var(--app-violet);
	}

	.pairing-status {
		display: grid;
		grid-template-columns: minmax(10rem, auto) minmax(0, 1fr);
		gap: 1rem;
		margin: -0.75rem 0 1.5rem;
		padding: 0.8rem 1rem;
		border: 1px solid var(--app-rule);
		background: var(--app-paper);
		font-size: 0.875rem;
		line-height: 1.45;
	}

	.pairing-status strong {
		color: var(--app-violet);
	}

	.pairing-status span {
		color: var(--app-muted);
	}

	.setup-grid {
		display: grid;
		grid-template-columns: minmax(30rem, 1.1fr) minmax(23rem, 0.9fr);
		gap: 1.5rem;
		align-items: start;
	}

	.setup-grid form,
	.setup-grid aside > section {
		border-top: 1px solid var(--app-ink);
		border-bottom: 1px solid var(--app-rule);
		background: var(--app-paper);
	}

	.setup-grid form > header,
	.setup-grid aside section > header {
		display: grid;
		grid-template-columns: 2rem 1fr;
		gap: 0.65rem;
		padding: 0.9rem 1rem;
		border-bottom: 1px solid var(--app-rule);
	}

	.setup-grid header > span {
		color: var(--app-violet);
		font: 0.64rem/1.4 'Fragment Mono', monospace;
	}

	.setup-grid header p {
		margin: 0.2rem 0 0;
		color: var(--app-muted);
		font-size: 0.875rem;
		line-height: 1.4;
	}

	fieldset {
		margin: 0;
		padding: 0;
		border: 0;
	}

	fieldset legend {
		padding: 1rem 1rem 0;
		font-size: 0.9rem;
		font-weight: 600;
	}

	.fieldset-help {
		margin: 0.2rem 0 0;
		padding: 0 1rem 0.8rem;
		border-bottom: 1px solid var(--app-rule);
		color: var(--app-muted);
		font-size: 0.875rem;
		line-height: 1.4;
	}

	fieldset label {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 0.8rem;
		align-items: start;
		padding: 1rem;
		border-bottom: 1px solid var(--app-rule);
		cursor: pointer;
	}

	fieldset label:hover {
		background: #fafaf7;
	}

	fieldset input {
		width: 1rem;
		height: 1rem;
		margin: 0.1rem 0 0;
		accent-color: var(--app-violet);
	}

	fieldset strong,
	fieldset small {
		display: block;
	}

	fieldset strong {
		font-size: 0.9rem;
	}

	fieldset strong i {
		margin-left: 0.4rem;
		padding: 0.15rem 0.3rem;
		background: var(--app-violet-soft);
		color: var(--app-violet);
		font: 0.56rem/1 'Fragment Mono', monospace;
		font-style: normal;
		text-transform: uppercase;
	}

	fieldset small {
		max-width: 42rem;
		margin-top: 0.25rem;
		color: var(--app-muted);
		font-size: 0.875rem;
		line-height: 1.4;
	}

	.planned-note {
		margin: 0;
		padding: 0.8rem 1rem;
		border-bottom: 1px solid var(--app-rule);
		background: #fff9df;
		color: var(--app-muted);
		font-size: 0.875rem;
		line-height: 1.4;
	}

	.planned-note strong {
		color: var(--app-warning);
	}

	form > button {
		width: 100%;
		padding: 0.9rem 1rem;
		border: 0;
		background: var(--app-ink);
		color: white;
		cursor: pointer;
		font: 600 0.875rem/1.2 'Source Sans 3', sans-serif;
	}

	form > button:hover {
		background: var(--app-violet);
	}

	form > button span {
		margin-left: 0.45rem;
	}

	.form-error {
		margin: 0;
		padding: 0.75rem 1rem;
		border-bottom: 1px solid #e3b8c0;
		background: #fff0f3;
		color: var(--app-danger);
		font-size: 0.875rem;
		font-weight: 600;
	}

	.setup-grid aside {
		display: grid;
		gap: 1.5rem;
	}

	.message-preview {
		margin: 1rem;
		padding: 0.85rem;
		border-radius: 6px;
		background: #eef6ec;
		color: #243124;
		font-family: system-ui, sans-serif;
	}

	.message-preview > div {
		display: flex;
		justify-content: space-between;
		gap: 1rem;
		margin-bottom: 0.65rem;
		font-size: 0.8rem;
	}

	.message-preview > div span {
		color: #9c3447;
		font-weight: 700;
	}

	.message-preview p {
		margin: 0.2rem 0;
		font-size: 0.875rem;
		line-height: 1.4;
	}

	.message-preview small {
		display: block;
		margin-top: 0.65rem;
		color: #657265;
		font-size: 0.8rem;
	}

	.billing-state {
		display: grid;
		gap: 0.2rem;
		padding: 0.9rem 1rem;
		background: #fff9df;
	}

	.billing-state strong {
		color: var(--app-warning);
		font-size: 0.875rem;
	}

	.billing-state strong.pilot-state {
		color: var(--app-success);
	}

	.billing-state span {
		color: var(--app-muted);
		font-size: 0.875rem;
	}

	.safety-note {
		max-width: 52rem;
		margin-top: 1.5rem;
		padding-top: 0.85rem;
		border-top: 1px solid var(--app-rule);
	}

	.safety-note strong {
		font-size: 0.875rem;
	}

	.safety-note p {
		margin: 0.2rem 0 0;
		color: var(--app-muted);
		font-size: 0.875rem;
		line-height: 1.45;
	}

	.message-state {
		max-width: 52rem;
		padding: 3rem 0;
	}

	.message-state .primary-button {
		display: inline-block;
		margin-top: 1.2rem;
	}

	@media (max-width: 980px) {
		.setup-header,
		.setup-grid {
			grid-template-columns: 1fr;
		}

		.setup-header dl {
			width: max-content;
			max-width: 100%;
		}
	}

	@media (max-width: 620px) {
		.progress {
			gap: 0.75rem;
		}

		.setup-header dl {
			display: grid;
			width: 100%;
		}

		.setup-header dl div {
			border-right: 0;
			border-bottom: 1px solid var(--app-rule);
		}

		.setup-header dl div:last-child {
			border-bottom: 0;
		}

		.telegram-ready {
			align-items: stretch;
			flex-direction: column;
		}

		.telegram-ready > a {
			text-align: center;
		}

		.pairing-status {
			grid-template-columns: 1fr;
			gap: 0.25rem;
		}
	}
</style>
