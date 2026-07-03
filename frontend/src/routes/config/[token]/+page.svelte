<script lang="ts">
	import { resolve } from '$app/paths';
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import { WalletButton, wallet } from '@fernolab/wallet-adapter-svelte';
	import { shortAddress } from '$lib/format';
	import { formatTimestamp } from '$lib/format';
	import type { PageData } from './$types';
	import type { PaymentLink, TelegramDeliveryMode, TelegramSubscription } from '$lib/types';

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

	type WatchMode = 'top100' | 'all' | 'custom';
	type ProtocolLevel = 'default' | 'critical' | 'high_impact';
	type SelectableProtocol = PageData['defiProtocols'][number];

	const TOP_PROTOCOL_LIMIT = 100;
	const watchModes: WatchMode[] = ['top100', 'all', 'custom'];
	let watchMode = $state<WatchMode>('top100');
	let protocolPickerOpen = $state(false);
	let protocolSearch = $state('');
	let selectedCustomProtocolSlugs = $state<string[]>([]);
	let defaultProtocolLevel = $state<ProtocolLevel>('default');

	const selectableProtocolList = $derived(selectableProtocols());
	const indexedProtocols = $derived.by(() => sortedProtocolsByIndex(selectableProtocolList));
	const topIndexedProtocols = $derived.by(() => indexedProtocols.slice(0, TOP_PROTOCOL_LIMIT));
	const selectedProtocolSlugs = $derived.by(() => {
		if (watchMode === 'all') return selectableProtocolList.map((protocol) => protocol.slug);
		if (watchMode === 'top100') return topIndexedProtocols.map((protocol) => protocol.slug);
		return selectedCustomProtocolSlugs;
	});
	const allProgramOptions = $derived(
		selectableProtocolList.flatMap((protocol) =>
			protocol.programs.map((program) => ({
				protocol,
				program
			}))
		)
	);
	const selectedCustomProtocols = $derived.by(() => {
		const protocolsBySlug = new Map(selectableProtocolList.map((protocol) => [protocol.slug, protocol]));
		return selectedCustomProtocolSlugs
			.map((slug) => protocolsBySlug.get(slug))
			.filter((protocol): protocol is SelectableProtocol => Boolean(protocol));
	});
	const filteredProtocolOptions = $derived.by(() => {
		const query = protocolSearch.trim().toLowerCase();
		if (!query) return indexedProtocols;
		return indexedProtocols.filter((protocol) => protocolSearchText(protocol).includes(query));
	});
	const groupedPayments = $derived(paymentGroups());

	function modeTitle(mode: WatchMode) {
		if (mode === 'all') return 'All';
		if (mode === 'top100') return 'Top 100';
		return 'Custom';
	}

	function modeCount(mode: WatchMode) {
		if (mode === 'all') return `${selectableProtocolList.length} protocols`;
		if (mode === 'top100') return `${topIndexedProtocols.length} protocols`;
		return `${selectedCustomProtocolSlugs.length} protocols`;
	}

	function modeHint(mode: WatchMode) {
		if (mode === 'all') return 'Every supported protocol';
		if (mode === 'top100') return 'Most complete coverage first';
		return 'Pick only what matters';
	}

	function setWatchMode(mode: WatchMode) {
		watchMode = mode;
		if (mode === 'custom') {
			protocolPickerOpen = true;
		}
	}

	function toggleCustomProtocol(slug: string) {
		if (selectedCustomProtocolSlugs.includes(slug)) {
			selectedCustomProtocolSlugs = selectedCustomProtocolSlugs.filter((candidate) => candidate !== slug);
		} else {
			selectedCustomProtocolSlugs = [...selectedCustomProtocolSlugs, slug];
		}
	}

	function removeCustomProtocol(slug: string) {
		selectedCustomProtocolSlugs = selectedCustomProtocolSlugs.filter((candidate) => candidate !== slug);
	}

	function protocolSourceCount(protocol: SelectableProtocol) {
		return protocol.programs.length + protocol.sol_domains.length + protocol.amount_triggers.length;
	}

	function protocolIndexScore(protocol: SelectableProtocol) {
		const coverageScore =
			protocol.coverage === 'deep_decode' ? 1000 : protocol.coverage === 'admin_signals' ? 600 : 200;
		return (
			coverageScore +
			protocol.catalog_instruction_count * 3 +
			protocol.high_impact_instruction_filters.length * 12 +
			protocol.admin_instruction_filters.length * 8 +
			protocol.amount_triggers.length * 8 +
			protocol.programs.length * 5 +
			(protocol.unknown_alerts ? 25 : 0)
		);
	}

	function sortedProtocolsByIndex(protocols: SelectableProtocol[]) {
		return [...protocols].sort((first, second) => {
			const scoreDelta = protocolIndexScore(second) - protocolIndexScore(first);
			if (scoreDelta !== 0) return scoreDelta;
			return first.name.localeCompare(second.name);
		});
	}

	function protocolSearchText(protocol: SelectableProtocol) {
		return [
			protocol.name,
			protocol.slug,
			protocol.program_id,
			protocol.coverage,
			...protocol.sol_domains,
			...protocol.programs.flatMap((program) => [program.label, program.program_id])
		]
			.join(' ')
			.toLowerCase();
	}

	function protocolLevel() {
		return defaultProtocolLevel;
	}

	function protocolLevelLabel(level: ProtocolLevel) {
		if (level === 'critical') return 'Critical only';
		if (level === 'high_impact') return 'High-impact admin';
		return 'Default DeFi';
	}

	function protocolCoverageLabel(protocol: SelectableProtocol) {
		if (protocol.coverage === 'deep_decode') return 'Decoded alerts';
		if (protocol.coverage === 'admin_signals') return 'Admin-change alerts';
		return 'Address signals';
	}

	function currentWatchLabel(subscription: TelegramSubscription) {
		if (subscription.scope === 'program_updates' && subscription.target_address) {
			const program = allProgramOptions.find(
				(option) => option.program.program_id === subscription.target_address
			);
			return program ? `${program.program.label} · ${program.protocol.name}` : shortAddress(subscription.target_address);
		}
		const protocolTag = subscription.required_tags.find((tag) => tag.startsWith('protocol:'));
		if (protocolTag) {
			const slug = protocolTag.replace('protocol:', '');
			return selectableProtocolList.find((protocol) => protocol.slug === slug)?.name ?? slug;
		}
		if (subscription.target_address) return shortAddress(subscription.target_address);
		return subscription.label;
	}

	function currentWatchScopeLabel(subscription: TelegramSubscription) {
		if (subscription.scope === 'defi_watch') return 'Protocol';
		if (subscription.scope === 'program_updates') return 'Program';
		if (subscription.scope === 'wallet_activity') return 'Wallet';
		return subscription.scope.replaceAll('_', ' ');
	}

	function currentWatchModeLabel(subscription: TelegramSubscription) {
		return deliveryModeLabel(subscription.delivery_mode);
	}

	function onPageKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			closeProtocolPicker();
		}
	}

	function closeProtocolPicker() {
		protocolPickerOpen = false;
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

	function planStatusLabel() {
		if (!data.billing) return 'Checkout required after save';
		return data.billing.active ? 'Active Custom Plan' : 'Checkout required after save';
	}

	function saveButtonLabel() {
		return data.billing?.active ? 'Save configuration' : 'Save and continue';
	}

	function saveOutcomeLabel() {
		return data.billing?.active
			? 'Changes apply immediately'
			: 'Daily summaries save now; checkout opens for live alerts';
	}

	function deliveryModeLabel(mode: TelegramDeliveryMode) {
		return mode === 'daily_summary' ? 'Daily summary' : 'Live alerts';
	}

	function deliveryModeHint(mode: TelegramDeliveryMode) {
		if (mode === 'daily_summary') return 'One Telegram digest per day';
		return data.billing?.active ? 'Send each alert as it lands' : 'Available after checkout';
	}

	function selectedCoverageLabel() {
		if (watchMode === 'custom') {
			return selectedCustomProtocols.length > 0
				? `${selectedCustomProtocols.length} custom protocol${selectedCustomProtocols.length === 1 ? '' : 's'}`
				: 'Custom protocols';
		}
		return `${modeTitle(watchMode)} index`;
	}

	function walletSummaryLabel() {
		if (walletPreview.invalid.length > 0) {
			return `${walletPreview.invalid.length} invalid wallet ${walletPreview.invalid.length === 1 ? 'entry' : 'entries'}`;
		}
		if (walletPreview.addresses.length === 0) return 'Protocol alerts only';
		return `${walletPreview.addresses.length} wallet${walletPreview.addresses.length === 1 ? '' : 's'} added`;
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

<svelte:window onkeydown={onPageKeydown} />

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
				description={`Private setup for ${userLabel()}. This link expires ${formatTimestamp(data.session.expires_at)}.`}
				level="1"
			/>
			<div class="session-grid">
				<div>
					<strong>Chat</strong>
					<span>{chatLabel()}</span>
				</div>
				<div>
					<strong>Plan</strong>
					<span>{planStatusLabel()}</span>
				</div>
				<div>
					<strong>Save</strong>
					<span>{saveOutcomeLabel()}</span>
				</div>
			</div>
		</section>

		<div class="setup-sticky">
			<div>
				<strong>{selectedCoverageLabel()}</strong>
				<span>{protocolLevelLabel(defaultProtocolLevel)} · {walletSummaryLabel()}</span>
			</div>
			<button type="submit" form="config-form">{saveButtonLabel()}</button>
		</div>

		{#if data.billing}
			<section class="panel account-panel">
				<div class="section-head">
					<h2>Plan status</h2>
					<p>{data.billing.active ? 'This chat can save changes immediately.' : 'Save the setup, then complete checkout to activate alerts.'}</p>
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

				{#if groupedPayments.pending.length > 0}
					<div class="payment-groups">
						<div class="payment-group">
							<h3>Pending checkout</h3>
							{#each groupedPayments.pending as link (link.id)}
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
					</div>
				{/if}
			</section>
		{/if}

		<form id="config-form" class="config-form" method="POST" action="?/save_config">
			<div class="config-layout">
				<div class="config-main">
					<section class="panel">
						<div class="section-head">
							<h2>Protocol coverage</h2>
							<p>Choose the index this chat should monitor. The full protocol list only opens for Custom.</p>
						</div>
						<div class="bucket-grid">
							{#each watchModes as mode (mode)}
								<button
									class:active={watchMode === mode}
									class="bucket-option"
									type="button"
									onclick={() => setWatchMode(mode)}
								>
									<strong>{modeTitle(mode)}</strong>
									<span>{modeCount(mode)}</span>
									<small>{modeHint(mode)}</small>
								</button>
							{/each}
						</div>
						{#each selectedProtocolSlugs as slug (slug)}
							<input type="hidden" name="protocol_slugs" value={slug} />
							<input type="hidden" name="protocol_levels" value={`${slug}:${protocolLevel()}`} />
						{/each}
						{#if watchMode === 'custom'}
							<div class="custom-summary">
								<div>
									<strong>{selectedCustomProtocols.length} custom protocols selected</strong>
									<span>
										{#if selectedCustomProtocols.length > 0}
											{selectedCustomProtocols.slice(0, 3).map((protocol) => protocol.name).join(', ')}
											{selectedCustomProtocols.length > 3 ? ` +${selectedCustomProtocols.length - 3} more` : ''}
										{:else}
											Search the full protocol list and pick only the teams this chat cares about.
										{/if}
									</span>
								</div>
								<button class="secondary-button" type="button" onclick={() => (protocolPickerOpen = true)}>
									Choose protocols
								</button>
							</div>
							{#if selectedCustomProtocols.length > 0}
								<div class="selected-protocols">
									{#each selectedCustomProtocols as protocol (protocol.slug)}
										<button
											class="selected-chip"
											type="button"
											onclick={() => removeCustomProtocol(protocol.slug)}
										>
											{protocol.name}
											<span>Remove</span>
										</button>
									{/each}
								</div>
							{/if}
						{/if}
					</section>

					<section class="panel">
						<div class="section-head">
							<h2>Alert level</h2>
							<p>One signal level applies across the selected protocol index.</p>
						</div>
						<div class="level-row">
							<div>
								<strong>
									{selectedProtocolSlugs.length}
									{selectedProtocolSlugs.length === 1 ? 'protocol' : 'protocols'} selected
								</strong>
								<span>{protocolLevelLabel(defaultProtocolLevel)}</span>
							</div>
							<select bind:value={defaultProtocolLevel} aria-label="Alert level for selected protocols">
								<option value="default">Default DeFi</option>
								<option value="critical">Critical only</option>
								<option value="high_impact">High-impact admin</option>
							</select>
						</div>
					</section>

					<section class="panel field-panel">
						<div class="section-head">
							<h2>Wallets</h2>
							<p>Optional. Add wallets or .sol names when the chat needs wallet-specific activity.</p>
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
				</div>

				<aside class="config-side">
					{#if (data.subscriptions ?? []).length > 0}
						<section class="panel">
							<div class="section-head">
								<h2>Current watches</h2>
								<p>Check a rule to remove it on save.</p>
							</div>
							<div class="watch-list">
								{#each data.subscriptions as subscription (subscription.id)}
									<label class="watch-row">
										<input type="checkbox" name="unsubscribe_ids" value={subscription.id} />
										<span>
											<strong>{currentWatchLabel(subscription)}</strong>
											<small>{currentWatchScopeLabel(subscription)} · {currentWatchModeLabel(subscription)} · {subscription.status.replaceAll('_', ' ')}</small>
										</span>
										<em>Remove</em>
									</label>
								{/each}
							</div>
						</section>
					{/if}

					<section id="billing" class="panel checkout-panel">
						<div class="section-head">
							<h2>{data.billing?.active ? 'Save changes' : 'Billing'}</h2>
							<p>{saveOutcomeLabel()}</p>
						</div>
						<div class="delivery-row" role="radiogroup" aria-label="Delivery mode">
							<label>
								<input
									type="radio"
									name="delivery_mode"
									value="daily_summary"
									checked={!data.billing?.active}
								/>
								<span>{deliveryModeLabel('daily_summary')}</span>
								<small>{deliveryModeHint('daily_summary')}</small>
							</label>
							<label class:disabled={!data.billing?.active}>
								<input
									type="radio"
									name="delivery_mode"
									value="live"
									checked={data.billing?.active}
									disabled={!data.billing?.active}
								/>
								<span>{deliveryModeLabel('live')}</span>
								<small>{deliveryModeHint('live')}</small>
							</label>
						</div>
						{#if !data.billing?.active}
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
									<span>Yearly</span>
									{#if billingAmountLine('year')}
										<small>{billingAmountLine('year')}</small>
									{/if}
								</label>
							</div>
						{/if}
						<div class="save-summary">
							<strong>{selectedCoverageLabel()}</strong>
							<span>{protocolLevelLabel(defaultProtocolLevel)} · {walletSummaryLabel()}</span>
						</div>
						<button type="submit">{saveButtonLabel()}</button>
					</section>
				</aside>
			</div>
		</form>

		{#if protocolPickerOpen}
			<div class="modal-backdrop" role="presentation" onclick={closeProtocolPicker}>
				<div
					class="program-modal"
					aria-label="Select custom protocols"
					aria-modal="true"
					role="dialog"
					tabindex="-1"
					onclick={(event) => event.stopPropagation()}
					onkeydown={(event) => event.stopPropagation()}
				>
					<header class="modal-head">
						<div>
							<h2>Custom protocols</h2>
							<p>{selectedCustomProtocols.length} selected · {filteredProtocolOptions.length} shown</p>
						</div>
						<button class="secondary-button" type="button" onclick={closeProtocolPicker}>
							Done
						</button>
					</header>
					<div class="program-filters">
						<label>
							<span>Search</span>
							<input
								bind:value={protocolSearch}
								placeholder="Protocol, program, or address"
								type="search"
							/>
						</label>
					</div>
					<div class="program-picker-list">
						{#each filteredProtocolOptions as protocol (protocol.slug)}
							{@const isSelected = selectedCustomProtocolSlugs.includes(protocol.slug)}
							<label class:tracked={isSelected} class="program-picker-row">
								<input
									checked={isSelected}
									type="checkbox"
									onchange={() => toggleCustomProtocol(protocol.slug)}
								/>
								<span>
									<ProtocolIdentity slug={protocol.slug} label={protocol.name} size="sm" />
									<small>
										{protocolCoverageLabel(protocol)} · {protocol.programs.length} programs ·
										{protocolSourceCount(protocol)} sources
									</small>
								</span>
								<em>{isSelected ? 'Selected' : 'Add'}</em>
							</label>
						{:else}
							<p class="empty-payments">No protocols match this search.</p>
						{/each}
					</div>
					{#if selectedCustomProtocols.length > 0}
						<div class="selected-protocols">
							{#each selectedCustomProtocols as protocol (protocol.slug)}
								<button
									class="selected-chip"
									type="button"
									onclick={() => removeCustomProtocol(protocol.slug)}
								>
									{protocol.name}
									<span>Remove</span>
								</button>
							{/each}
						</div>
					{/if}
				</div>
			</div>
		{/if}

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

	.setup-sticky {
		position: sticky;
		top: calc(var(--fw-header-height, 0px) + 0.5rem);
		z-index: 4;
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.68rem;
		padding: 0.72rem;
		border: 1px solid rgba(189, 74, 23, 0.18);
		border-radius: 10px;
		background: rgba(255, 252, 248, 0.96);
		box-shadow: 0 10px 26px rgba(23, 32, 42, 0.08);
		backdrop-filter: blur(10px);
	}

	.setup-sticky div {
		display: grid;
		gap: 0.08rem;
		min-width: 0;
	}

	.setup-sticky span {
		color: #5f6c79;
		font-size: 0.82rem;
		font-weight: 700;
	}

	.config-layout {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(18rem, 24rem);
		gap: 1rem;
		align-items: start;
	}

	.config-main,
	.config-side {
		display: grid;
		gap: 1rem;
		min-width: 0;
	}

	.config-side {
		align-self: start;
	}

	.session-grid,
	.billing-summary-grid,
	.field-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 0.7rem;
	}

	.field-grid {
		grid-template-columns: repeat(2, minmax(0, 1fr));
	}

	.session-grid div,
	.billing-summary-grid div {
		display: grid;
		gap: 0.12rem;
		padding: 0.72rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		background: #fbfaf8;
	}

	.bucket-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 0.7rem;
	}

	.bucket-option {
		justify-self: stretch;
		display: grid;
		gap: 0.18rem;
		min-height: 4.4rem;
		padding: 0.72rem;
		border-color: rgba(23, 32, 42, 0.1);
		background: #fbfaf8;
		color: #17202a;
		text-align: left;
	}

	.bucket-option.active {
		border-color: #bd4a17;
		background: rgba(189, 74, 23, 0.08);
	}

	.bucket-option strong,
	.custom-summary strong,
	.watch-row strong {
		overflow-wrap: anywhere;
	}

	.bucket-option span,
	.bucket-option small,
	.custom-summary span,
	.level-row span,
	.watch-row em,
	.program-picker-row em,
	.selected-chip span {
		color: #5f6c79;
		font-size: 0.78rem;
		font-weight: 750;
	}

	.custom-summary,
	.level-row,
	.watch-row,
	.program-picker-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.75rem;
		padding: 0.68rem 0.72rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		background: #fbfaf8;
	}

	.custom-summary div,
	.level-row div,
	.watch-row span,
	.program-picker-row span,
	.modal-head div {
		display: grid;
		gap: 0.16rem;
		min-width: 0;
	}

	.watch-list {
		display: grid;
		gap: 0.55rem;
	}

	.level-row select {
		min-width: 12rem;
	}

	.watch-row {
		grid-template-columns: auto minmax(0, 1fr) auto;
		cursor: pointer;
	}

	.watch-row input {
		accent-color: #bd4a17;
	}

	.watch-row em,
	.program-picker-row em {
		font-style: normal;
	}

	.billing-row input,
	.delivery-row input {
		margin-top: 0.2rem;
		accent-color: #bd4a17;
	}

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

	.payment-row div {
		display: grid;
		gap: 0.1rem;
		min-width: 0;
	}

	.payment-row strong,
	.payment-row span {
		overflow-wrap: anywhere;
	}

	.payment-row > a {
		flex: 0 0 auto;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 2.25rem;
		padding: 0.35rem 0.52rem;
		border-radius: 8px;
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

	input[type='search'],
	select,
	textarea {
		width: 100%;
		min-height: 2.65rem;
		padding: 0.68rem;
		border-radius: 8px;
		border: 1px solid rgba(23, 32, 42, 0.12);
		background: #fff;
		color: #17202a;
		font: inherit;
		font-size: 0.9rem;
		resize: vertical;
	}

	input[type='search']:focus,
	select:focus,
	textarea:focus {
		outline: 2px solid rgba(189, 74, 23, 0.2);
		border-color: rgba(189, 74, 23, 0.55);
	}

	.modal-backdrop {
		position: fixed;
		inset: 0;
		z-index: 20;
		display: grid;
		place-items: center;
		padding: 1rem;
		background: rgba(23, 32, 42, 0.36);
	}

	.program-modal {
		width: min(58rem, 100%);
		max-height: min(42rem, calc(100vh - 2rem));
		display: grid;
		grid-template-rows: auto auto minmax(0, 1fr) auto;
		gap: 0.85rem;
		padding: 1rem;
		border: 1px solid rgba(23, 32, 42, 0.12);
		border-radius: 10px;
		background: #fff;
		box-shadow: 0 8px 24px rgba(23, 32, 42, 0.18);
	}

	.modal-head {
		display: flex;
		justify-content: space-between;
		gap: 0.75rem;
		align-items: start;
	}

	.modal-head p {
		color: #5f6c79;
		font-size: 0.82rem;
		font-weight: 700;
	}

	.program-filters {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: 0.65rem;
	}

	.program-picker-list {
		min-height: 0;
		overflow: auto;
		display: grid;
		gap: 0.5rem;
		padding-right: 0.15rem;
	}

	.program-picker-row {
		grid-template-columns: auto minmax(0, 1fr) auto;
	}

	.program-picker-row.tracked {
		border-color: rgba(189, 74, 23, 0.24);
		background: rgba(189, 74, 23, 0.06);
	}

	.program-picker-row input {
		accent-color: #bd4a17;
	}

	.selected-protocols {
		display: flex;
		flex-wrap: wrap;
		gap: 0.45rem;
		padding-top: 0.2rem;
		border-top: 1px solid rgba(23, 32, 42, 0.08);
	}

	.selected-chip {
		min-height: 2rem;
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
		border-color: rgba(23, 32, 42, 0.14);
		background: #fff;
		color: #17202a;
	}

	.billing-row,
	.delivery-row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.7rem;
	}

	.billing-row label,
	.delivery-row label {
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

	.delivery-row label.disabled {
		opacity: 0.64;
	}

	.billing-row label span,
	.billing-row label small,
	.delivery-row label span,
	.delivery-row label small {
		grid-column: 2;
	}

	.billing-row label small,
	.delivery-row label small {
		margin: 0;
	}

	.save-summary {
		display: grid;
		gap: 0.12rem;
		padding: 0.68rem 0.72rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		background: #fbfaf8;
	}

	.save-summary strong,
	.save-summary span {
		min-width: 0;
		overflow-wrap: anywhere;
	}

	.save-summary span {
		color: #5f6c79;
		font-size: 0.78rem;
		font-weight: 750;
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
		.page-shell {
			width: min(var(--fw-page-max), calc(100vw - 1rem));
			padding: 0.75rem 0 8rem;
			padding-bottom: calc(8rem + env(safe-area-inset-bottom));
			gap: 0.75rem;
		}

		.panel {
			padding: 0.86rem;
			border-radius: 8px;
		}

		.intro-panel,
		.field-panel,
		.checkout-panel,
		.config-main,
		.config-side {
			gap: 0.75rem;
		}

		.setup-sticky {
			position: fixed;
			right: 0.75rem;
			bottom: 0.75rem;
			bottom: calc(0.75rem + env(safe-area-inset-bottom));
			left: 0.75rem;
			z-index: 30;
			grid-template-columns: 1fr;
			align-items: stretch;
			gap: 0.52rem;
			padding: 0.72rem;
			box-shadow: 0 6px 18px rgba(23, 32, 42, 0.14);
			backdrop-filter: none;
		}

		.setup-sticky span {
			font-size: 0.8rem;
		}

		.setup-sticky button {
			width: 100%;
			min-height: 3rem;
			justify-self: stretch;
		}

		.session-grid,
		.billing-summary-grid,
		.bucket-grid,
		.config-layout,
		.program-filters,
		.field-grid {
			grid-template-columns: 1fr;
		}

		button {
			min-height: 3rem;
			justify-self: stretch;
		}

		.secondary-button {
			min-height: 2.75rem;
		}

		input[type='search'],
		select,
		textarea {
			min-width: 0;
			font-size: 16px;
		}

		.wallet-toolbar,
		.wallet-connect,
		.wallet-actions {
			display: grid;
			grid-template-columns: 1fr;
			width: 100%;
		}

		.wallet-connect > *,
		.wallet-actions > * {
			width: 100%;
		}

		.bucket-option {
			min-height: 3.4rem;
			padding: 0.74rem;
		}

		.custom-summary,
		.level-row {
			grid-template-columns: 1fr;
			align-items: stretch;
		}

		.level-row select {
			min-width: 0;
		}

		.watch-row,
		.program-picker-row,
		.billing-row label,
		.delivery-row label {
			min-height: 3rem;
		}

		.billing-row,
		.delivery-row {
			display: grid;
			grid-template-columns: 1fr;
		}

		.selected-chip {
			min-height: 2.75rem;
		}

		.modal-backdrop {
			padding: 0;
			place-items: stretch;
		}

		.program-modal {
			width: 100%;
			max-height: 100vh;
			max-height: 100dvh;
			min-height: 100vh;
			min-height: 100dvh;
			padding: 0.85rem;
			border-radius: 0;
			border-width: 0;
		}

		.program-picker-list {
			overscroll-behavior: contain;
			-webkit-overflow-scrolling: touch;
			padding-right: 0;
		}

		.payment-row {
			align-items: start;
			flex-direction: column;
		}

		.payment-row > a {
			width: 100%;
			min-height: 2.75rem;
		}
	}
</style>
