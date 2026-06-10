<script lang="ts">
	import { resolve } from '$app/paths';
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import { WalletButton, wallet } from '@fernolab/wallet-adapter-svelte';
	import { shortAddress } from '$lib/format';
	import { formatTimestamp } from '$lib/format';
	import type { PageData } from './$types';
	import type { PaymentLink } from '$lib/types';

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

	function billing() {
		return data.billing ?? null;
	}

	function paymentLinks() {
		return billing()?.payment_links ?? [];
	}

	function tokenForPayment(link: { spl_token: string | null }) {
		if (!link.spl_token) return 'SOL';
		return link.spl_token === 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v' ? 'USDC' : 'SPL';
	}

	function planLabel(planId: string) {
		if (planId === 'paid-year') return 'Yearly custom watcher';
		if (planId === 'paid') return 'Monthly custom watcher';
		return planId.replaceAll('-', ' ');
	}

	function paymentDisplayStatus(link: { status: string; expires_at: number }) {
		if (link.status === 'pending' && link.expires_at <= Math.floor(Date.now() / 1000)) {
			return 'timed_out';
		}
		return link.status;
	}

	function paymentStatusLabel(status: string) {
		if (status === 'timed_out') return 'Timed out';
		if (status === 'paid') return 'Paid';
		if (status === 'pending') return 'Pending';
		return status.replaceAll('_', ' ');
	}

	function paymentToken(publicUrl: string) {
		try {
			const url = new URL(publicUrl);
			const token = url.pathname.match(/^\/pay\/([^/]+)$/)?.[1];
			if (token) return token;
		} catch {
			const token = publicUrl.match(/^\/pay\/([^/]+)$/)?.[1];
			if (token) return token;
		}
		return '';
	}

	function paymentGroups() {
		const groups = {
			pending: [],
			paid: [],
			timedOut: []
		} as {
			pending: PaymentLink[];
			paid: PaymentLink[];
			timedOut: PaymentLink[];
		};

		for (const link of paymentLinks()) {
			const status = paymentDisplayStatus(link);
			if (status === 'paid') groups.paid.push(link);
			else if (status === 'timed_out' || status === 'expired' || status === 'canceled') {
				groups.timedOut.push(link);
			} else if (status === 'pending') {
				groups.pending.push(link);
			}
		}
		return groups;
	}

	function durationLabel(seconds: number) {
		if (seconds <= 0) return 'No active time';
		const days = Math.ceil(seconds / (24 * 60 * 60));
		if (days >= 60) return `${Math.ceil(days / 30)} months`;
		if (days >= 2) return `${days} days`;
		return `${days} day`;
	}

	function balanceLabel() {
		const summary = billing();
		if (!summary) return 'No payment history';
		if (summary.balance_months <= 0) return 'No paid balance';
		return `${summary.balance_months} month${summary.balance_months === 1 ? '' : 's'}`;
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

	function pricingToken() {
		return data.customPlanPricing?.token ?? 'USDC';
	}

	function billingAmount(kind: 'month' | 'year') {
		if (kind === 'month') return data.customPlanPricing?.monthly ?? null;
		return data.customPlanPricing?.yearly ?? null;
	}

	function billingAmountLine(kind: 'month' | 'year') {
		const amount = billingAmount(kind);
		if (!amount) return null;
		return `${amount} ${pricingToken()}`;
	}

	type WalletAddressbookItem = {
		address: string;
		name: string;
		bundleIds: string[];
	};

	type WalletAddressbookBundle = {
		id: string;
		name: string;
	};

	const DEFAULT_WALLET_BUNDLE_ID = 'FW_MAIN';
	const DEFAULT_WALLET_BUNDLE_NAME = 'Main';
	const SOLANA_ADDRESS_PATTERN = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;

	let walletAddressesValue = $state('');
	let walletImportInput = $state<HTMLInputElement | null>(null);
	let walletImportMessage = $state<string | null>(null);
	let walletImportError = $state<string | null>(null);
	let walletNamesByAddress = $state<Record<string, string>>({});
	let walletBundleIdsByAddress = $state<Record<string, string[]>>({});
	let walletBundleNames = $state<Record<string, string>>({
		[DEFAULT_WALLET_BUNDLE_ID]: DEFAULT_WALLET_BUNDLE_NAME
	});

	function normalizedWalletAddress(value: string) {
		const trimmed = value.trim();
		if (!trimmed) return null;
		return SOLANA_ADDRESS_PATTERN.test(trimmed) ? trimmed : null;
	}

	function parseWalletAddressLines(raw: string) {
		const tokens = raw
			.split(/[\n,]+/)
			.map((value) => value.trim())
			.filter(Boolean);
		const seen = new Set<string>();
		const addresses: string[] = [];
		const invalid: string[] = [];

		for (const token of tokens) {
			const normalized = normalizedWalletAddress(token);
			if (!normalized) {
				invalid.push(token);
				continue;
			}
			if (seen.has(normalized)) continue;
			seen.add(normalized);
			addresses.push(normalized);
		}

		return { addresses, invalid };
	}

	function setWalletAddressList(addresses: string[]) {
		walletAddressesValue = addresses.join('\n');
	}

	function addWalletAddress(
		rawAddress: string,
		options?: { name?: string | null; bundleIds?: string[] | null }
	) {
		const normalized = normalizedWalletAddress(rawAddress);
		if (!normalized) {
			throw new Error(`Invalid Solana wallet address: ${rawAddress}`);
		}

		const { addresses, invalid } = parseWalletAddressLines(walletAddressesValue);
		if (invalid.length > 0) {
			throw new Error(`Fix ${invalid.length} invalid wallet address entries first.`);
		}

		if (!addresses.includes(normalized)) {
			addresses.push(normalized);
			setWalletAddressList(addresses);
		}

		const nextName = options?.name?.trim();
		if (nextName) {
			walletNamesByAddress = { ...walletNamesByAddress, [normalized]: nextName };
		}

		const nextBundleIds = options?.bundleIds?.map((value) => value.trim()).filter(Boolean);
		if (nextBundleIds?.length) {
			walletBundleIdsByAddress = {
				...walletBundleIdsByAddress,
				[normalized]: nextBundleIds
			};
			walletBundleNames = {
				...walletBundleNames,
				...Object.fromEntries(nextBundleIds.map((id) => [id, walletBundleNames[id] || id]))
			};
		}
	}

	function addConnectedWallet() {
		if (!wallet.connected || !wallet.publicKey) {
			walletImportMessage = null;
			walletImportError = 'Connect a wallet first.';
			return;
		}

		try {
			addWalletAddress(wallet.publicKey, {
				name: wallet.standardWallet?.name ?? 'connected-wallet'
			});
			walletImportError = null;
			walletImportMessage = `Connected wallet ${shortAddress(wallet.publicKey)} added.`;
		} catch (error) {
			walletImportMessage = null;
			walletImportError =
				error instanceof Error ? error.message : 'Unable to add the connected wallet.';
		}
	}

	function walletPreviewRows() {
		const { addresses, invalid } = parseWalletAddressLines(walletAddressesValue);
		return {
			addresses,
			invalid
		};
	}

	const walletPreview = $derived(walletPreviewRows());

	function parseAddressbookJson(raw: string) {
		const payload = JSON.parse(raw) as unknown;
		const itemRecords = Array.isArray(payload)
			? payload
			: Array.isArray((payload as { items?: unknown } | null)?.items)
				? ((payload as { items: unknown[] }).items ?? [])
				: null;
		const bundleRecords =
			!Array.isArray(payload) && Array.isArray((payload as { bundles?: unknown } | null)?.bundles)
				? ((payload as { bundles: unknown[] }).bundles ?? [])
				: [];

		if (!itemRecords) {
			throw new Error('Addressbook import failed: missing items array.');
		}

		const seen = new Set<string>();
		const items: WalletAddressbookItem[] = [];
		const invalid: string[] = [];

		for (const [index, entry] of itemRecords.entries()) {
			if (!entry || typeof entry !== 'object') continue;
			const record = entry as {
				address?: unknown;
				name?: unknown;
				bundleIds?: unknown;
			};
			const address = normalizedWalletAddress(String(record.address ?? ''));
			if (!address) {
				invalid.push(String(record.address ?? ''));
				continue;
			}
			if (seen.has(address)) continue;
			seen.add(address);

			const bundleIds = Array.isArray(record.bundleIds)
				? record.bundleIds
						.map((value) => String(value).trim())
						.filter(Boolean)
				: [];

			items.push({
				address,
				name: String(record.name ?? '').trim() || `wallet-${index + 1}`,
				bundleIds: bundleIds.length ? bundleIds : [DEFAULT_WALLET_BUNDLE_ID]
			});
		}

		const bundles: WalletAddressbookBundle[] = bundleRecords
			.filter((value): value is Record<string, unknown> => Boolean(value) && typeof value === 'object')
			.map((entry) => ({
				id: String(entry.id ?? '').trim(),
				name: String(entry.name ?? '').trim()
			}))
			.filter((entry) => entry.id.length > 0);

		return { items, bundles, invalid };
	}

	function importWalletAddressbookFromText(raw: string) {
		const { items, bundles, invalid } = parseAddressbookJson(raw);
		if (items.length === 0) {
			throw new Error('Addressbook import failed: no valid wallet addresses found.');
		}
		if (invalid.length > 0) {
			throw new Error(`Addressbook import failed: ${invalid.length} invalid wallet address entries.`);
		}

		const nextNames: Record<string, string> = {};
		const nextBundleIds: Record<string, string[]> = {};
		const nextBundleNames: Record<string, string> = {
			[DEFAULT_WALLET_BUNDLE_ID]: DEFAULT_WALLET_BUNDLE_NAME
		};

		for (const bundle of bundles) {
			nextBundleNames[bundle.id] = bundle.name || bundle.id;
		}

		for (const item of items) {
			nextNames[item.address] = item.name;
			nextBundleIds[item.address] = item.bundleIds;
			for (const bundleId of item.bundleIds) {
				if (!nextBundleNames[bundleId]) {
					nextBundleNames[bundleId] = bundleId;
				}
			}
		}

		walletNamesByAddress = nextNames;
		walletBundleIdsByAddress = nextBundleIds;
		walletBundleNames = nextBundleNames;
		setWalletAddressList(items.map((item) => item.address));
		walletImportError = null;
		walletImportMessage = `${items.length} wallet${items.length === 1 ? '' : 's'} imported from addressbook.`;
	}

	function walletItemsForExport() {
		const { addresses, invalid } = parseWalletAddressLines(walletAddressesValue);
		if (invalid.length > 0) {
			throw new Error(`Cannot export: ${invalid.length} invalid wallet address entries.`);
		}
		if (addresses.length === 0) {
			throw new Error('Cannot export: add at least one wallet address.');
		}

		return addresses.map((address, index) => {
			const name = walletNamesByAddress[address]?.trim() || `wallet-${index + 1}`;
			const bundleIds = walletBundleIdsByAddress[address]?.length
				? walletBundleIdsByAddress[address]
				: [DEFAULT_WALLET_BUNDLE_ID];
			return { address, name, bundleIds };
		});
	}

	function exportWalletAddressbook() {
		try {
			const items = walletItemsForExport();
			const usedBundleIds = Array.from(new Set(items.flatMap((item) => item.bundleIds)));
			const bundles = usedBundleIds.map((id) => ({
				id,
				name: walletBundleNames[id] || id
			}));

			const payload = {
				version: '2.0.0',
				items,
				bundles,
				createdAt: Date.now()
			};

			const blob = new Blob([JSON.stringify(payload, null, 2)], {
				type: 'application/json'
			});
			const href = URL.createObjectURL(blob);
			const link = document.createElement('a');
			link.href = href;
			link.download = `firewatch-wallet-addressbook-${new Date().toISOString().slice(0, 10)}.json`;
			link.click();
			URL.revokeObjectURL(href);

			walletImportError = null;
			walletImportMessage = `${items.length} wallet${items.length === 1 ? '' : 's'} exported.`;
		} catch (error) {
			walletImportMessage = null;
			walletImportError =
				error instanceof Error ? error.message : 'Wallet export failed due to invalid data.';
		}
	}

	function openWalletImport() {
		walletImportInput?.click();
	}

	async function onWalletImportChange(event: Event & { currentTarget: EventTarget & HTMLInputElement }) {
		const file = event.currentTarget.files?.[0];
		if (!file) return;

		try {
			const raw = await file.text();
			importWalletAddressbookFromText(raw);
		} catch (error) {
			walletImportMessage = null;
			walletImportError =
				error instanceof Error ? error.message : 'Wallet import failed: invalid file.';
		} finally {
			event.currentTarget.value = '';
		}
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

		{#if data.billing}
			<section class="panel account-panel">
				<div class="section-head">
					<h2>Account and payments</h2>
					<p>Track pending checkouts, paid invoices, timed-out links, and Custom Plan time.</p>
				</div>
				<div class="billing-summary-grid">
					<div>
						<strong>Access</strong>
						<span class:active-access={data.billing.active}>
							{data.billing.active ? 'Active' : 'Not active'}
						</span>
					</div>
					<div>
						<strong>Balance</strong>
						<span>{balanceLabel()}</span>
						<small>{data.billing.credit_months} paid · {data.billing.consumed_months} used</small>
					</div>
					<div>
						<strong>Remaining</strong>
						<span>{durationLabel(data.billing.remaining_seconds)}</span>
						{#if data.billing.next_payment_due_at}
							<small>Due {formatTimestamp(data.billing.next_payment_due_at)}</small>
						{:else}
							<small>No next payment due</small>
						{/if}
					</div>
				</div>

				{#if paymentLinks().length > 0}
					{@const groups = paymentGroups()}
					<div class="payment-groups">
						{#if groups.pending.length > 0}
							<div class="payment-group">
								<h3>Pending payments</h3>
								{#each groups.pending as link (link.id)}
									{@const checkoutToken = paymentToken(link.public_url)}
									<div class="payment-row">
										<div>
											<strong>{link.amount} {tokenForPayment(link)}</strong>
											<span>{planLabel(link.plan_id)} · expires {formatTimestamp(link.expires_at)}</span>
										</div>
											<a href={resolve('/pay/[token]', { token: checkoutToken })}>Open payment</a>
									</div>
								{/each}
							</div>
						{/if}
						{#if groups.paid.length > 0}
							<div class="payment-group">
								<h3>Paid payments</h3>
								{#each groups.paid.slice(0, 5) as link (link.id)}
									<div class="payment-row">
										<div>
											<strong>{link.amount} {tokenForPayment(link)}</strong>
											<span>{planLabel(link.plan_id)} · paid {formatTimestamp(link.paid_at ?? link.created_at)}</span>
										</div>
										<span class="payment-status status-paid">{paymentStatusLabel('paid')}</span>
									</div>
								{/each}
							</div>
						{/if}
						{#if groups.timedOut.length > 0}
							<div class="payment-group">
								<h3>Timed-out payments</h3>
								{#each groups.timedOut.slice(0, 3) as link (link.id)}
									<div class="payment-row muted">
										<div>
											<strong>{link.amount} {tokenForPayment(link)}</strong>
											<span>{planLabel(link.plan_id)} · ended {formatTimestamp(link.expires_at)}</span>
										</div>
										<span class="payment-status">{paymentStatusLabel(paymentDisplayStatus(link))}</span>
									</div>
								{/each}
							</div>
						{/if}
					</div>
				{:else}
					<p class="empty-payments">No payment links yet. Save a Custom Plan watch to create checkout.</p>
				{/if}
			</section>
		{/if}

		<form class="config-form" method="POST" action="?/save_config">
			<section class="panel">
				<div class="section-head">
					<h2>Protocols</h2>
					<p>Select every protocol this chat should watch.</p>
				</div>
				<div class="protocol-grid">
					{#each selectableProtocols() as protocol (protocol.slug)}
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
					{#each featuredPrograms() as item (`${item.protocol.slug}:${item.program.program_id}`)}
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
					<p>Add one address or name per line. Import/export supports Jupiter addressbook format.</p>
				</div>
				<div class="wallet-toolbar">
					<div class="wallet-connect">
						<WalletButton />
						<button
							class="secondary-button"
							type="button"
							onclick={addConnectedWallet}
							disabled={!wallet.connected}
						>
							Add connected wallet
						</button>
					</div>
					<div class="wallet-actions">
						<button class="secondary-button" type="button" onclick={openWalletImport}>Import JSON</button>
						<button class="secondary-button" type="button" onclick={exportWalletAddressbook}>Export JSON</button>
						<input
							bind:this={walletImportInput}
							class="wallet-import-input"
							type="file"
							accept=".json,application/json"
							onchange={onWalletImportChange}
						/>
					</div>
				</div>
				{#if wallet.connected && wallet.publicKey}
					<p class="wallet-status">
						Connected: <code>{wallet.publicKey}</code>
					</p>
				{/if}
				{#if walletImportError}
					<p class="wallet-feedback error">{walletImportError}</p>
				{:else if walletImportMessage}
					<p class="wallet-feedback success">{walletImportMessage}</p>
				{/if}
				<div class="field-grid">
					<label>
						<span>Wallet addresses</span>
						<textarea
							bind:value={walletAddressesValue}
							name="wallet_addresses"
							rows="5"
							placeholder="Wallet pubkey"
						></textarea>
						{#if walletPreview.invalid.length > 0}
							<small class="wallet-inline-error">
								{walletPreview.invalid.length} invalid wallet
								{walletPreview.invalid.length === 1 ? '' : 's'} detected.
							</small>
						{:else if walletPreview.addresses.length > 0}
							<small>
								{walletPreview.addresses.length} wallet
								{walletPreview.addresses.length === 1 ? '' : 's'} ready.
							</small>
							<ul class="wallet-preview-list">
								{#each walletPreview.addresses.slice(0, 6) as address (address)}
									<li>
										<code>{shortAddress(address)}</code>
										{#if walletNamesByAddress[address]}
											<span>{walletNamesByAddress[address]}</span>
										{/if}
									</li>
								{/each}
								{#if walletPreview.addresses.length > 6}
									<li>+{walletPreview.addresses.length - 6} more</li>
								{/if}
							</ul>
						{/if}
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
						<p>
							Custom bot configuration is a paid feature. Existing Custom Plan users save immediately.
							Otherwise, selected watches are saved as pending and checkout opens next.
						</p>
					</div>
					<div class="billing-row">
						<label>
							<input type="radio" name="billing" value="month" checked />
							<span>Monthly</span>
							{#if billingAmountLine('month')}
								<small>{billingAmountLine('month')}</small>
							{/if}
						</label>
						<label>
							<input type="radio" name="billing" value="year" />
							<span>Yearly (1 month discount)</span>
							{#if billingAmountLine('year')}
								<small>{billingAmountLine('year')}</small>
							{/if}
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
	.billing-summary-grid,
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
	.billing-summary-grid div,
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
	.session-grid span,
	.billing-summary-grid strong,
	.billing-summary-grid span,
	.billing-summary-grid small {
		min-width: 0;
		overflow-wrap: anywhere;
	}

	.account-panel {
		display: grid;
		gap: 0.85rem;
	}

	.billing-summary-grid span {
		font-size: 1.05rem;
		font-weight: 900;
		color: #17202a;
	}

	.billing-summary-grid .active-access {
		color: #32704c;
	}

	.payment-groups {
		display: grid;
		gap: 0.7rem;
	}

	.payment-group {
		display: grid;
		gap: 0.45rem;
	}

	.payment-group h3 {
		margin: 0;
		font-size: 0.86rem;
	}

	.payment-row {
		display: flex;
		justify-content: space-between;
		gap: 0.75rem;
		align-items: center;
		padding: 0.68rem 0.75rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		background: #fbfaf8;
	}

	.payment-row.muted {
		background: #fff;
		color: #5f6c79;
	}

	.payment-row div {
		display: grid;
		gap: 0.1rem;
		min-width: 0;
	}

	.payment-row strong,
	.payment-row span {
		overflow-wrap: anywhere;
	}

	.payment-row > a,
	.payment-status {
		flex: 0 0 auto;
		padding: 0.35rem 0.52rem;
		border-radius: 999px;
		border: 1px solid rgba(23, 32, 42, 0.1);
		background: #fff;
		color: #17202a;
		font-size: 0.76rem;
		font-weight: 850;
		text-decoration: none;
	}

	.payment-row > a {
		border-color: #bd4a17;
		background: #bd4a17;
		color: #fff;
	}

	.status-paid {
		border-color: rgba(50, 112, 76, 0.18);
		background: rgba(50, 112, 76, 0.08);
		color: #32704c;
	}

	.empty-payments {
		margin: 0;
		padding: 0.7rem 0.75rem;
		border: 1px dashed rgba(23, 32, 42, 0.16);
		border-radius: 8px;
		color: #5f6c79;
	}

	.section-head,
	label {
		display: grid;
		gap: 0.32rem;
	}

	.wallet-toolbar,
	.wallet-connect,
	.wallet-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.55rem;
		align-items: center;
	}

	.wallet-toolbar {
		justify-content: space-between;
	}

	.wallet-status {
		margin: 0;
		color: #5f6c79;
		font-size: 0.82rem;
	}

	.wallet-status code {
		color: #17202a;
		font-size: 0.78rem;
	}

	.wallet-feedback {
		margin: 0;
		padding: 0.55rem 0.65rem;
		border-radius: 8px;
		font-size: 0.82rem;
		font-weight: 700;
	}

	.wallet-feedback.error {
		border: 1px solid rgba(156, 57, 41, 0.18);
		background: rgba(156, 57, 41, 0.08);
		color: #9c3929;
	}

	.wallet-feedback.success {
		border: 1px solid rgba(50, 112, 76, 0.18);
		background: rgba(50, 112, 76, 0.08);
		color: #32704c;
	}

	.wallet-import-input {
		display: none;
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

	.wallet-inline-error {
		color: #9c3929;
	}

	.wallet-preview-list {
		margin: 0;
		padding: 0;
		list-style: none;
		display: grid;
		gap: 0.25rem;
	}

	.wallet-preview-list li {
		display: flex;
		flex-wrap: wrap;
		gap: 0.35rem;
		align-items: center;
		font-size: 0.76rem;
		color: #5f6c79;
	}

	.wallet-preview-list code {
		color: #17202a;
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
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		align-items: start;
		column-gap: 0.45rem;
		row-gap: 0.08rem;
		padding: 0.58rem 0.72rem;
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 8px;
		background: #fbfaf8;
	}

	.billing-row label span,
	.billing-row label small {
		grid-column: 2;
	}

	.billing-row label small {
		margin: 0;
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

	button:disabled {
		cursor: not-allowed;
		opacity: 0.6;
	}

	.secondary-button {
		min-height: 2rem;
		padding: 0 0.7rem;
		border: 1px solid rgba(23, 32, 42, 0.14);
		background: #fff;
		color: #17202a;
		font-size: 0.78rem;
		font-weight: 700;
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
		.billing-summary-grid,
		.protocol-grid,
		.program-grid,
		.field-grid {
			grid-template-columns: 1fr;
		}

		.payment-row {
			align-items: start;
			flex-direction: column;
		}
	}
</style>
