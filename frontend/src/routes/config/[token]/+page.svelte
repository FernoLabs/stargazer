<script lang="ts">
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import { shortAddress } from '$lib/format';
	import { formatTimestamp } from '$lib/format';
	import type { PageData } from './$types';

	let {
		data,
		form
	}: {
		data: PageData;
		form: { error?: string; message?: string } | null;
	} = $props();

	function userLabel() {
		if (!data.user) return 'Telegram admin';
		if (data.user.telegram_username) return `@${data.user.telegram_username}`;
		if (data.user.telegram_user_id) return String(data.user.telegram_user_id);
		return data.user.label ?? 'Telegram admin';
	}

	function chatLabel() {
		if (data.session?.chat_type === 'private') return 'Direct message';
		return data.session?.chat_title ?? 'Telegram chat';
	}

	function selectableProtocols() {
		return data.defiProtocols.filter(
			(protocol) =>
				protocol.programs.length > 0 ||
				protocol.sol_domains.length > 0 ||
				protocol.amount_triggers.length > 0
		);
	}

	function featuredPrograms() {
		return data.defiProtocols
			.flatMap((protocol) =>
				protocol.programs.slice(0, 3).map((program) => ({
					protocol,
					program
				}))
			)
			.slice(0, 18);
	}
</script>

<svelte:head>
	<title>Configure FireWatch</title>
</svelte:head>

<div class="page-shell">
	{#if data.error || !data.session || !data.user}
		<section class="panel">
			<SectionHeader
				title="Config link expired"
				description={data.error ?? 'Run /config again in Telegram to receive a fresh private setup link.'}
				level="1"
				href="/"
				hrefLabel="Home"
			/>
		</section>
	{:else}
		<section class="panel intro-panel">
			<SectionHeader
				title={`Configure ${chatLabel()}`}
				description={`Signed in from Telegram as ${userLabel()}. This setup link expires ${formatTimestamp(data.session.expires_at)}.`}
				level="1"
			/>
			<div class="session-grid">
				<div>
					<strong>Admin</strong>
					<span>{userLabel()}</span>
				</div>
				<div>
					<strong>Target</strong>
					<span>{chatLabel()}</span>
				</div>
				<div>
					<strong>Flow</strong>
					<span>Save config, then pay if Custom Plan is required</span>
				</div>
			</div>
		</section>

		<form class="config-form" method="POST" action="?/save_config">
			<section class="panel">
				<div class="section-head">
					<h2>Protocols</h2>
					<p>Select every protocol this chat should watch.</p>
				</div>
				<div class="protocol-grid">
					{#each selectableProtocols() as protocol}
						<label class="option-row">
							<input type="checkbox" name="protocol_slugs" value={protocol.slug} />
							<span>
								<ProtocolIdentity slug={protocol.slug} label={protocol.name} size="sm" />
								<small>
									{protocol.programs.length} programs
									{#if protocol.amount_triggers.length > 0}
										· {protocol.amount_triggers.length} amount triggers
									{/if}
								</small>
							</span>
						</label>
					{/each}
				</div>
			</section>

			<section class="panel">
				<div class="section-head">
					<h2>Programs</h2>
					<p>Optional program-level watches for specific deployments or upgrades.</p>
				</div>
				<div class="program-grid">
					{#each featuredPrograms() as item}
						<label class="option-row">
							<input type="checkbox" name="program_ids" value={item.program.program_id} />
							<span>
								<strong>{item.program.label}</strong>
								<small>{item.protocol.name} · {shortAddress(item.program.program_id)}</small>
							</span>
						</label>
					{/each}
				</div>
			</section>

			<section class="panel field-panel">
				<div class="section-head">
					<h2>Wallets and .sol names</h2>
					<p>Add one address or name per line. Wallet rules activate on transfers.</p>
				</div>
				<div class="field-grid">
					<label>
						<span>Wallet addresses</span>
						<textarea name="wallet_addresses" rows="5" placeholder="Wallet pubkey"></textarea>
					</label>
					<label>
						<span>.sol names</span>
						<textarea name="sol_domains" rows="5" placeholder="ferno.sol"></textarea>
					</label>
				</div>
			</section>

			<section class="panel checkout-panel">
				<div class="section-head">
					<h2>Billing</h2>
					<p>Existing Custom Plan users save immediately. Otherwise, the selected watches are saved as pending and the next screen opens payment.</p>
				</div>
				<div class="billing-row">
					<label>
						<input type="radio" name="billing" value="month" checked />
						<span>Monthly</span>
					</label>
					<label>
						<input type="radio" name="billing" value="year" />
						<span>Yearly, one month discount</span>
					</label>
				</div>
				<button type="submit">Save configuration</button>
			</section>
		</form>

		{#if form?.error}
			<p class="flash error">{form.error}</p>
		{:else if form?.message}
			<p class="flash success">{form.message}</p>
		{/if}
	{/if}
</div>

<style>
	.page-shell {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem 0 2.4rem;
		display: grid;
		gap: 1rem;
	}

	.config-form,
	.intro-panel,
	.field-panel,
	.checkout-panel {
		display: grid;
		gap: 1rem;
	}

	.panel {
		padding: 1.05rem;
		background: #fff;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 10px;
	}

	.session-grid,
	.protocol-grid,
	.program-grid,
	.field-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 0.7rem;
	}

	.protocol-grid,
	.program-grid {
		grid-template-columns: repeat(2, minmax(0, 1fr));
	}

	.field-grid {
		grid-template-columns: repeat(2, minmax(0, 1fr));
	}

	.session-grid div,
	.option-row {
		display: grid;
		gap: 0.12rem;
		padding: 0.72rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		background: #fbfaf8;
	}

	.option-row {
		grid-template-columns: auto minmax(0, 1fr);
		align-items: start;
		gap: 0.65rem;
		cursor: pointer;
	}

	.option-row input,
	.billing-row input {
		margin-top: 0.2rem;
		accent-color: #bd4a17;
	}

	.option-row span,
	.option-row small,
	.session-grid strong,
	.session-grid span {
		min-width: 0;
		overflow-wrap: anywhere;
	}

	.section-head,
	label {
		display: grid;
		gap: 0.32rem;
	}

	h2,
	p {
		margin: 0;
	}

	h2 {
		font-size: 1.05rem;
	}

	.section-head p,
	.session-grid span,
	small {
		color: #5f6c79;
		line-height: 1.45;
	}

	small {
		font-size: 0.78rem;
		font-weight: 700;
	}

	label > span {
		font-size: 0.86rem;
		font-weight: 800;
	}

	textarea {
		width: 100%;
		padding: 0.68rem;
		border-radius: 8px;
		border: 1px solid rgba(23, 32, 42, 0.12);
		background: #fff;
		color: #17202a;
		font: inherit;
		font-size: 0.9rem;
		resize: vertical;
	}

	textarea:focus {
		outline: 2px solid rgba(189, 74, 23, 0.2);
		border-color: rgba(189, 74, 23, 0.55);
	}

	.billing-row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.7rem;
	}

	.billing-row label {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
		padding: 0.58rem 0.72rem;
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 8px;
		background: #fbfaf8;
	}

	button {
		justify-self: start;
		min-height: 2.35rem;
		padding: 0 0.88rem;
		border: 1px solid #bd4a17;
		border-radius: 8px;
		background: #bd4a17;
		color: #fff;
		font: inherit;
		font-size: 0.84rem;
		font-weight: 800;
		cursor: pointer;
	}

	.flash {
		padding: 0.75rem 0.85rem;
		border-radius: 8px;
		font-weight: 700;
	}

	.flash.error {
		border: 1px solid rgba(156, 57, 41, 0.18);
		background: rgba(156, 57, 41, 0.08);
		color: #9c3929;
	}

	.flash.success {
		border: 1px solid rgba(50, 112, 76, 0.18);
		background: rgba(50, 112, 76, 0.08);
		color: #32704c;
	}

	@media (max-width: 840px) {
		.session-grid,
		.protocol-grid,
		.program-grid,
		.field-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
