<script lang="ts">
	import { resolve } from '$app/paths';
	import SectionHeader from '$lib/components/SectionHeader.svelte';
	import ProtocolIdentity from '$lib/components/ProtocolIdentity.svelte';
	import { WalletButton, wallet } from '@fernolab/wallet-adapter-svelte';
	import {
		canonicalProtocolSlug,
		hasProtocolListedItems,
		protocolListedItemCount,
		uniqueProtocolGovernanceCount
	} from '$lib/catalog';
	import {
		formatMetricValue,
		formatTimestamp,
		readableInstructionName,
		shortAddress
	} from '$lib/format';
	import type { PageData } from './$types';
	import type {
		MetricAlertPreset,
		PaymentLink,
		TelegramDeliveryMode,
		TelegramSubscription,
		WalletHistoryScanResponse
	} from '$lib/types';

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
		return data.defiProtocols.filter(hasProtocolListedItems);
	}

	type ProtocolLevel = 'normal' | 'high' | 'critical';
	type ProtocolSource = 'current' | 'curated' | 'manual' | 'history' | 'live';
	type ProtocolSourceFilter = 'all' | ProtocolSource;
	type SelectableProtocol = PageData['defiProtocols'][number];

	const CURATED_PROTOCOL_LIMIT = 100;
	const PAYMENT_LINK_PREVIEW_LIMIT = 3;
	const WALLET_HISTORY_SCAN_LIMIT = 100;
	const USER_PROTOCOL_PREVIEW_LIMIT = 8;
	const SELECTED_PROTOCOL_PREVIEW_LIMIT = 12;
	const PROTOCOL_SOURCE_FILTER_ORDER: ProtocolSource[] = [
		'history',
		'live',
		'manual',
		'curated',
		'current'
	];
	const SOLANA_ADDRESS_PATTERN = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;
	const SOL_DOMAIN_PATTERN = /^[a-z0-9][a-z0-9-]{0,62}\.sol$/i;
	const initialSelectedProtocolSlugs = initialProtocolSlugsFromSubscriptions();
	const initialWalletAddressList = initialWalletAddressesFromSubscriptions();
	const initialSolDomainList = initialSolDomainsFromSubscriptions();
	const initialBroadDefiWatch = hasBroadDefiSubscription();
	const initialCuratedProtocolSlugs = initialBroadDefiWatch ? curatedProtocolSlugsFromCatalog() : [];
	let protocolPickerOpen = $state(false);
	let clearConfigConfirmOpen = $state(false);
	let clearAllConfirmed = $state(false);
	let protocolSearch = $state('');
	let selectedCustomProtocolSlugs = $state<string[]>(
		initialSelectedProtocolSlugs.length > 0 ? initialSelectedProtocolSlugs : initialCuratedProtocolSlugs
	);
	let protocolSourcesBySlug = $state<Record<string, ProtocolSource>>(initialProtocolSources());
	let defaultProtocolLevel = $state<ProtocolLevel>('normal');
	let customProtocolLevels = $state<Record<string, ProtocolLevel>>(initialProtocolLevelsFromSubscriptions());
	let protocolProgramIdsBySlug = $state<Record<string, string[]>>(
		initialProtocolProgramIdsBySlugFromSubscriptions()
	);
	let activeProtocolSlug = $state<string | null>(null);
	let selectedProtocolSearch = $state('');
	let selectedProtocolSourceFilter = $state<ProtocolSourceFilter>('all');
	let selectedProtocolListExpanded = $state(false);
	let protocolRuleMessage = $state<string | null>(
		initialBroadDefiWatch ? 'Curated preset loaded from the existing /watch defi rule.' : null
	);
	let metricPresetKeysByTrigger = $state<Record<string, string>>({});
	let autoDiscoveryMessage = $state<string | null>(null);
	let walletScanState = $state<'idle' | 'scanning' | 'error'>('idle');
	let walletScanError = $state<string | null>(null);
	let dailyDeliveryEnabled = $state(true);
	let liveDeliveryEnabled = $state(true);

	const selectableProtocolList = $derived(selectableProtocols());
	const currentSubscriptionCount = $derived(data.subscriptions?.length ?? 0);
	const protocolsBySlug = $derived.by(
		() => new Map(selectableProtocolList.map((protocol) => [protocol.slug, protocol]))
	);
	const loadedWalletAddressList = $derived.by(initialWalletAddressesFromSubscriptions);
	const loadedSolDomainList = $derived.by(initialSolDomainsFromSubscriptions);
	const loadedWalletConfigKey = $derived(
		`${loadedWalletAddressList.join('|')}::${loadedSolDomainList.join('|')}`
	);
	const indexedProtocols = $derived.by(() => sortedProtocolsByIndex(selectableProtocolList));
	const curatedProtocols = $derived.by(() => indexedProtocols.slice(0, CURATED_PROTOCOL_LIMIT));
	const selectedProtocolSlugs = $derived(selectedCustomProtocolSlugs);
	const selectedProtocols = $derived.by(() =>
		selectedProtocolSlugs
			.map((slug) => protocolsBySlug.get(slug))
			.filter((protocol): protocol is SelectableProtocol => Boolean(protocol))
	);
	const autoDiscoveredProtocols = $derived.by(() =>
		selectedProtocols.filter((protocol) => {
			const source = protocolSource(protocol.slug);
			return source === 'history' || source === 'live';
		})
	);
	const userProtocolPreview = $derived.by(() =>
		[...selectedProtocols]
			.sort((left, right) => {
				const sourceDelta =
					protocolSourceRank(protocolSource(left.slug)) -
					protocolSourceRank(protocolSource(right.slug));
				if (sourceDelta !== 0) return sourceDelta;
				return left.name.localeCompare(right.name);
			})
			.slice(0, USER_PROTOCOL_PREVIEW_LIMIT)
	);
	const hiddenUserProtocolCount = $derived(
		Math.max(0, selectedProtocols.length - userProtocolPreview.length)
	);
	const selectedProtocolSourceFilters = $derived.by(() => {
		const sources = new Set(selectedProtocols.map((protocol) => protocolSource(protocol.slug)));
		return [
			'all' as ProtocolSourceFilter,
			...PROTOCOL_SOURCE_FILTER_ORDER.filter((source) => sources.has(source))
		];
	});
	const filteredSelectedProtocols = $derived.by(() => {
		const query = selectedProtocolSearch.trim().toLowerCase();
		return selectedProtocols.filter((protocol) => {
			const source = protocolSource(protocol.slug);
			const matchesSource =
				selectedProtocolSourceFilter === 'all' || source === selectedProtocolSourceFilter;
			const matchesQuery = !query || protocolSearchText(protocol).includes(query);
			return matchesSource && matchesQuery;
		});
	});
	const displayedSelectedProtocols = $derived.by(() =>
		selectedProtocolListExpanded
			? filteredSelectedProtocols
			: filteredSelectedProtocols.slice(0, SELECTED_PROTOCOL_PREVIEW_LIMIT)
	);
	const hiddenSelectedProtocolCount = $derived(
		Math.max(0, filteredSelectedProtocols.length - displayedSelectedProtocols.length)
	);
	const protocolSlugsForSave = $derived.by(() =>
		selectedProtocols
			.filter((protocol) => !hasProtocolProgramOverride(protocol.slug))
			.map((protocol) => protocol.slug)
	);
	const protocolProgramIdsForSave = $derived.by(() =>
		selectedProtocols.flatMap((protocol) =>
			hasProtocolProgramOverride(protocol.slug)
				? selectedProgramIdsForProtocol(protocol).map((programId) => `${protocol.slug}:${programId}`)
				: []
		)
	);
	const protocolProgramSlugsForSave = $derived.by(() =>
		Array.from(new Set(protocolProgramIdsForSave.map((entry) => entry.split(':')[0]).filter(Boolean)))
	);
	const allProgramOptions = $derived(
		selectableProtocolList.flatMap((protocol) =>
			protocol.programs.map((program) => ({
				protocol,
				program
			}))
		)
	);
	const filteredProtocolOptions = $derived.by(() => {
		const query = protocolSearch.trim().toLowerCase();
		if (!query) return indexedProtocols;
		return indexedProtocols.filter((protocol) => protocolSearchText(protocol).includes(query));
	});
	const protocolPickerOptions = $derived.by(() =>
		filteredProtocolOptions.filter((protocol) => !selectedCustomProtocolSlugs.includes(protocol.slug))
	);

	function subscriptionProtocolSlug(subscription: TelegramSubscription) {
		if (subscription.scope !== 'defi_watch') return null;
		const protocolTag = subscription.required_tags.find((tag) => tag.startsWith('protocol:'));
		const slug = protocolTag?.replace('protocol:', '').trim();
		return slug ? canonicalProtocolSlug(slug) : null;
	}

	function hasBroadDefiSubscription() {
		return (data.subscriptions ?? []).some(
			(subscription) =>
				subscription.scope === 'defi_watch' &&
				!subscription.target_address &&
				!subscription.required_tags.some((tag) => tag.startsWith('protocol:')) &&
				!subscription.required_tags.some((tag) => tag.startsWith('sol_domain:'))
		);
	}

	function subscriptionLevel(subscription: TelegramSubscription): ProtocolLevel {
		if (subscription.required_tags.includes('severity:critical')) return 'critical';
		if (subscription.required_tags.includes('impact:high')) return 'high';
		return 'normal';
	}

	function initialProtocolSlugsFromSubscriptions() {
		return Array.from(
			new Set(
				(data.subscriptions ?? [])
					.map(subscriptionProtocolSlug)
					.filter((slug): slug is string => Boolean(slug))
			)
		);
	}

	function initialProtocolLevelsFromSubscriptions() {
		const levels: Record<string, ProtocolLevel> = {};
		for (const subscription of data.subscriptions ?? []) {
			const slug = subscriptionProtocolSlug(subscription);
			if (!slug) continue;
			const nextLevel = subscriptionLevel(subscription);
			const currentLevel = levels[slug] ?? 'normal';
			if (currentLevel === 'critical') continue;
			if (currentLevel === 'high' && nextLevel === 'normal') continue;
			levels[slug] = nextLevel;
		}
		return levels;
	}

	function initialProtocolProgramIdsBySlugFromSubscriptions() {
		const wholeProtocolSlugs = new Set<string>();
		const programIdsBySlug = new Map<string, string[]>();

		for (const subscription of data.subscriptions ?? []) {
			const slug = subscriptionProtocolSlug(subscription);
			if (!slug) continue;
			if (!subscription.target_address) {
				wholeProtocolSlugs.add(slug);
				continue;
			}
			const programIds = programIdsBySlug.get(slug) ?? [];
			programIds.push(subscription.target_address);
			programIdsBySlug.set(slug, programIds);
		}

		return Object.fromEntries(
			Array.from(programIdsBySlug.entries())
				.filter(([slug]) => !wholeProtocolSlugs.has(slug))
				.map(([slug, programIds]) => [slug, Array.from(new Set(programIds))])
		);
	}

	function initialWalletAddressesFromSubscriptions() {
		return Array.from(
			new Set(
				(data.subscriptions ?? [])
					.filter((subscription) => subscription.scope === 'wallet_activity')
					.map((subscription) => subscription.target_address?.trim() ?? '')
					.filter((address) => SOLANA_ADDRESS_PATTERN.test(address))
			)
		);
	}

	function initialSolDomainsFromSubscriptions() {
		return Array.from(
			new Set(
				(data.subscriptions ?? []).flatMap((subscription) =>
					subscription.required_tags
						.filter((tag) => tag.startsWith('sol_domain:'))
						.map((tag) => tag.replace('sol_domain:', '').trim())
						.filter((domain) => SOL_DOMAIN_PATTERN.test(domain))
				)
			)
		);
	}

	function curatedProtocolSlugsFromCatalog() {
		return sortedProtocolsByIndex(selectableProtocols())
			.slice(0, CURATED_PROTOCOL_LIMIT)
			.map((protocol) => protocol.slug);
	}

	function initialProtocolSources() {
		const sources: Record<string, ProtocolSource> = {};
		for (const subscription of data.subscriptions ?? []) {
			const slug = subscriptionProtocolSlug(subscription);
			if (!slug) continue;
			const source = protocolSourceFromSubscription(subscription);
			if (source === 'live' || !sources[slug]) {
				sources[slug] = source;
			}
		}
		for (const slug of initialCuratedProtocolSlugs) {
			if (!sources[slug]) sources[slug] = 'curated';
		}
		return sources;
	}

	function protocolSourceFromSubscription(subscription: TelegramSubscription): ProtocolSource {
		const label = subscription.label.toLowerCase();
		if (label.startsWith('auto /watch protocol')) return 'live';
		if (label.startsWith('history /watch protocol')) return 'history';
		return 'current';
	}

	function useCuratedPreset() {
		const nextSlugs = curatedProtocols.map((protocol) => protocol.slug);
		selectedCustomProtocolSlugs = nextSlugs;
		customProtocolLevels = {
			...Object.fromEntries(nextSlugs.map((slug) => [slug, 'normal' as ProtocolLevel])),
			...customProtocolLevels
		};
		protocolSourcesBySlug = Object.fromEntries(
			nextSlugs.map((slug) => [slug, 'curated' as ProtocolSource])
		);
		activeProtocolSlug = null;
		selectedProtocolSearch = '';
		selectedProtocolSourceFilter = 'all';
		selectedProtocolListExpanded = false;
		protocolRuleMessage = `${nextSlugs.length} curated protocol rules selected. You can remove or configure them before saving.`;
	}

	function addAllProtocols() {
		const allSlugs = indexedProtocols.map((protocol) => protocol.slug);
		const selected = new Set(selectedCustomProtocolSlugs);
		const newSlugs = allSlugs.filter((slug) => !selected.has(slug));

		selectedCustomProtocolSlugs = allSlugs;
		customProtocolLevels = {
			...Object.fromEntries(newSlugs.map((slug) => [slug, defaultProtocolLevel])),
			...customProtocolLevels
		};
		protocolSourcesBySlug = {
			...Object.fromEntries(newSlugs.map((slug) => [slug, 'manual' as ProtocolSource])),
			...protocolSourcesBySlug
		};
		activeProtocolSlug = null;
		selectedProtocolSearch = '';
		selectedProtocolSourceFilter = 'all';
		selectedProtocolListExpanded = false;
		clearAllConfirmed = false;
		protocolRuleMessage =
			newSlugs.length === 0
				? `${allSlugs.length} protocols are already selected.`
				: `${newSlugs.length} protocol ${newSlugs.length === 1 ? 'rule' : 'rules'} added. ${allSlugs.length} total selected.`;
	}

	function openClearConfigConfirm() {
		clearConfigConfirmOpen = true;
	}

	function closeClearConfigConfirm() {
		clearConfigConfirmOpen = false;
	}

	function clearAllConfig() {
		selectedCustomProtocolSlugs = [];
		customProtocolLevels = {};
		protocolSourcesBySlug = {};
		protocolProgramIdsBySlug = {};
		activeProtocolSlug = null;
		selectedProtocolSearch = '';
		selectedProtocolSourceFilter = 'all';
		selectedProtocolListExpanded = false;
		protocolSearch = '';
		protocolPickerOpen = false;
		clearConfigConfirmOpen = false;
		clearAllConfirmed = true;
		setWalletAddressList([]);
		setSolDomainList([]);
		watchTargetInput = '';
		walletNamesByAddress = {};
		walletBundleIdsByAddress = {};
		walletBundleNames = {
			[DEFAULT_WALLET_BUNDLE_ID]: DEFAULT_WALLET_BUNDLE_NAME
		};
		walletImportError = null;
		walletImportMessage = null;
		walletScanState = 'idle';
		walletScanError = null;
		autoDiscoveryMessage = null;
		protocolRuleMessage =
			currentSubscriptionCount > 0
				? 'All draft config cleared. Save cleared config to remove current watch rules.'
				: 'Draft config cleared.';
	}

	function openProtocolPicker() {
		protocolPickerOpen = true;
	}

	async function scanWalletHistory() {
		if (watchTargetCount === 0) {
			autoDiscoveryMessage = 'Add a wallet or .sol name first.';
			walletScanError = null;
			return;
		}

		if (walletPreview.invalid.length > 0 || solDomainPreview.invalid.length > 0) {
			walletScanState = 'error';
			walletScanError = 'Fix invalid wallet or .sol entries before scanning.';
			autoDiscoveryMessage = null;
			return;
		}

		walletScanState = 'scanning';
		walletScanError = null;
		autoDiscoveryMessage = 'Scanning recent wallet activity...';

		try {
			const response = await fetch(`${window.location.pathname.replace(/\/$/, '')}/scan-wallets`, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({
					wallet_addresses: walletPreview.addresses,
					sol_domains: solDomainPreview.domains,
					limit: WALLET_HISTORY_SCAN_LIMIT
				})
			});
			const payload = (await response.json()) as WalletHistoryScanResponse & { error?: string };
			if (!response.ok) {
				throw new Error(payload.error ?? 'Wallet scan failed.');
			}

			const discoveredProtocols = payload.protocols
				.map((hit) => protocolsBySlug.get(canonicalProtocolSlug(hit.slug)))
				.filter((protocol): protocol is SelectableProtocol => Boolean(protocol));
			const discoveredSlugs = discoveredProtocols.map((protocol) => protocol.slug);
			const newSlugs = discoveredSlugs.filter((slug) => !selectedCustomProtocolSlugs.includes(slug));
			if (discoveredSlugs.length === 0) {
				autoDiscoveryMessage =
					payload.scanned_wallet_count === 0
						? 'No protocol recommendations from the current .sol targets.'
						: `No protocol recommendations from ${payload.transaction_count} scanned transactions.`;
				walletScanState = 'idle';
				return;
			}

			if (newSlugs.length === 0) {
				protocolSourcesBySlug = {
					...protocolSourcesBySlug,
					...Object.fromEntries(
						discoveredSlugs.map((slug) => [
							slug,
							protocolSource(slug) === 'current' ? 'current' : ('history' as ProtocolSource)
						])
					)
				};
				autoDiscoveryMessage = `${discoveredSlugs.length} matched protocol ${discoveredSlugs.length === 1 ? 'is' : 'are'} already in the watchlist.`;
				walletScanState = 'idle';
				return;
			}

			selectedCustomProtocolSlugs = [
				...newSlugs,
				...selectedCustomProtocolSlugs.filter((slug) => !newSlugs.includes(slug))
			];
			customProtocolLevels = {
				...Object.fromEntries(newSlugs.map((slug) => [slug, 'normal' as ProtocolLevel])),
				...customProtocolLevels
			};
			protocolSourcesBySlug = {
				...protocolSourcesBySlug,
				...Object.fromEntries(discoveredSlugs.map((slug) => [slug, 'history' as ProtocolSource]))
			};
			activeProtocolSlug = newSlugs[0] ?? activeProtocolSlug;
			selectedProtocolSearch = '';
			selectedProtocolSourceFilter = 'all';
			selectedProtocolListExpanded = false;
			protocolRuleMessage = null;
			autoDiscoveryMessage = `${newSlugs.length} discovered protocol ${newSlugs.length === 1 ? 'rule' : 'rules'} added from ${payload.transaction_count} scanned transactions.`;
			walletScanState = 'idle';
		} catch (error) {
			walletScanState = 'error';
			walletScanError = error instanceof Error ? error.message : 'Wallet scan failed.';
			autoDiscoveryMessage = null;
		}
	}

	function toggleCustomProtocol(slug: string) {
		if (selectedCustomProtocolSlugs.includes(slug)) {
			selectedCustomProtocolSlugs = selectedCustomProtocolSlugs.filter((candidate) => candidate !== slug);
			const remaining = { ...customProtocolLevels };
			delete remaining[slug];
			customProtocolLevels = remaining;
			const remainingSources = { ...protocolSourcesBySlug };
			delete remainingSources[slug];
			protocolSourcesBySlug = remainingSources;
			removeProtocolProgramOverride(slug);
			if (activeProtocolSlug === slug) activeProtocolSlug = null;
			protocolRuleMessage = null;
		} else {
			selectedCustomProtocolSlugs = [...selectedCustomProtocolSlugs, slug];
			customProtocolLevels = { ...customProtocolLevels, [slug]: defaultProtocolLevel };
			protocolSourcesBySlug = { ...protocolSourcesBySlug, [slug]: 'manual' };
			activeProtocolSlug = slug;
			selectedProtocolSearch = '';
			selectedProtocolSourceFilter = 'all';
			selectedProtocolListExpanded = true;
			protocolRuleMessage = null;
		}
	}

	function removeCustomProtocol(slug: string) {
		selectedCustomProtocolSlugs = selectedCustomProtocolSlugs.filter((candidate) => candidate !== slug);
		const remaining = { ...customProtocolLevels };
		delete remaining[slug];
		customProtocolLevels = remaining;
		const remainingSources = { ...protocolSourcesBySlug };
		delete remainingSources[slug];
		protocolSourcesBySlug = remainingSources;
		removeProtocolProgramOverride(slug);
		if (activeProtocolSlug === slug) activeProtocolSlug = null;
		protocolRuleMessage = null;
	}

	function protocolSource(slug: string): ProtocolSource {
		return protocolSourcesBySlug[slug] ?? 'manual';
	}

	function protocolSourceName(source: ProtocolSource) {
		if (source === 'current') return 'Existing';
		if (source === 'curated') return 'Curated';
		if (source === 'history') return 'Discovered';
		if (source === 'live') return 'Auto-added';
		return 'Manual';
	}

	function protocolSourceRank(source: ProtocolSource) {
		if (source === 'live') return 0;
		if (source === 'history') return 1;
		if (source === 'current') return 2;
		if (source === 'manual') return 3;
		return 4;
	}

	function protocolSourceLabel(slug: string) {
		return protocolSourceName(protocolSource(slug));
	}

	function protocolSourceFilterLabel(source: ProtocolSourceFilter) {
		return source === 'all' ? 'All' : protocolSourceName(source);
	}

	function protocolSourceFilterCount(source: ProtocolSourceFilter) {
		if (source === 'all') return selectedProtocols.length;
		return selectedProtocols.filter((protocol) => protocolSource(protocol.slug) === source).length;
	}

	function setCustomProtocolLevel(slug: string, level: ProtocolLevel) {
		customProtocolLevels = { ...customProtocolLevels, [slug]: level };
	}

	function onCustomProtocolLevelChange(
		slug: string,
		event: Event & { currentTarget: EventTarget & HTMLSelectElement }
	) {
		setCustomProtocolLevel(slug, event.currentTarget.value as ProtocolLevel);
	}

	function metricPresetKey(preset: MetricAlertPreset) {
		return [
			preset.protocol_slug,
			preset.instruction_name,
			preset.metric_key,
			preset.subject_address ?? ''
		].join('|');
	}

	function metricFieldName(key: string, field: string) {
		return `metric_rule:${field}:${key}`;
	}

	function metricKeyForAmountTrigger(label: string) {
		const normalized = label.toLowerCase();
		if (normalized.includes('token')) return 'token_exchange_price';
		if (normalized.includes('liquidity')) return 'liquidity_exchange_price';
		return null;
	}

	function metricTriggerKey(protocol: SelectableProtocol, triggerLabel: string) {
		return `${protocol.slug}:${metricKeyForAmountTrigger(triggerLabel) ?? triggerLabel.toLowerCase().replaceAll(/\s+/g, '_')}`;
	}

	function metricPresetsForTrigger(protocol: SelectableProtocol, triggerLabel: string) {
		const metricKey = metricKeyForAmountTrigger(triggerLabel);
		if (!metricKey) return [];
		return data.metricAlertPresets.filter(
			(preset) => preset.protocol_slug === protocol.slug && preset.metric_key === metricKey
		);
	}

	function selectedMetricPresetForTrigger(protocol: SelectableProtocol, triggerLabel: string) {
		const triggerKey = metricTriggerKey(protocol, triggerLabel);
		const presets = metricPresetsForTrigger(protocol, triggerLabel);
		if (presets.length === 0) return null;
		const selectedKey = metricPresetKeysByTrigger[triggerKey];
		return presets.find((preset) => metricPresetKey(preset) === selectedKey) ?? presets[0];
	}

	function onMetricPresetChange(
		key: string,
		event: Event & { currentTarget: EventTarget & HTMLSelectElement }
	) {
		metricPresetKeysByTrigger = {
			...metricPresetKeysByTrigger,
			[key]: event.currentTarget.value
		};
	}

	function metricSubjectLabel(preset: MetricAlertPreset) {
		if (preset.subject_label) return preset.subject_label;
		if (preset.subject_address) return shortAddress(preset.subject_address);
		return 'All observed subjects';
	}

	function hasSavedMetricRule(preset: MetricAlertPreset) {
		return data.metricAlertRules.some(
			(rule) =>
				rule.protocol_slug === preset.protocol_slug &&
				rule.instruction_name === preset.instruction_name &&
				rule.metric_key === preset.metric_key &&
				rule.subject_address === preset.subject_address
		);
	}

	function protocolSourceCount(protocol: SelectableProtocol) {
		return protocolListedItemCount(protocol);
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
			...protocol.programs.flatMap((program) => [program.label, program.program_id]),
			...(protocol.related_governances ?? []).flatMap((link) => [
				link.realm_name,
				link.realm,
				link.governance,
				link.governed_account
			])
		]
			.join(' ')
			.toLowerCase();
	}

	function protocolLevel(level: ProtocolLevel) {
		if (level === 'critical') return 'critical';
		if (level === 'high') return 'high_impact';
		return 'default';
	}

	function selectedProtocolLevel(slug: string) {
		return customProtocolLevels[slug] ?? defaultProtocolLevel;
	}

	function openProtocolDetails(slug: string) {
		activeProtocolSlug = activeProtocolSlug === slug ? null : slug;
	}

	function protocolProgramIds(protocol: SelectableProtocol) {
		return protocol.programs.map((program) => program.program_id);
	}

	function selectedProgramIdsForProtocol(protocol: SelectableProtocol) {
		return protocolProgramIdsBySlug[protocol.slug] ?? protocolProgramIds(protocol);
	}

	function hasProtocolProgramOverride(slug: string) {
		return Object.hasOwn(protocolProgramIdsBySlug, slug);
	}

	function removeProtocolProgramOverride(slug: string) {
		if (!hasProtocolProgramOverride(slug)) return;
		const next = { ...protocolProgramIdsBySlug };
		delete next[slug];
		protocolProgramIdsBySlug = next;
	}

	function isProtocolProgramSelected(protocol: SelectableProtocol, programId: string) {
		return selectedProgramIdsForProtocol(protocol).includes(programId);
	}

	function toggleProtocolProgram(protocol: SelectableProtocol, programId: string) {
		const allProgramIds = protocolProgramIds(protocol);
		const selected = new Set(selectedProgramIdsForProtocol(protocol));
		if (selected.has(programId)) selected.delete(programId);
		else selected.add(programId);
		const nextSelected = allProgramIds.filter((candidate) => selected.has(candidate));
		if (nextSelected.length === allProgramIds.length) {
			removeProtocolProgramOverride(protocol.slug);
			return;
		}
		protocolProgramIdsBySlug = {
			...protocolProgramIdsBySlug,
			[protocol.slug]: nextSelected
		};
	}

	function selectedProtocolItemSummary(protocol: SelectableProtocol) {
		const selectedCount = selectedProgramIdsForProtocol(protocol).length;
		const totalPrograms = protocol.programs.length;
		if (!hasProtocolProgramOverride(protocol.slug)) return protocolItemSummary(protocol);
		return `${selectedCount} of ${totalPrograms} programs selected`;
	}

	function protocolLevelLabel(level: ProtocolLevel) {
		if (level === 'critical') return 'Critical';
		if (level === 'high') return 'High';
		return 'Normal';
	}

	function protocolLevelHint(level: ProtocolLevel) {
		if (level === 'critical') return 'Only critical alerts';
		if (level === 'high') return 'High and critical alerts';
		return 'Normal, high, and critical alerts';
	}

	function pluralCount(count: number, singular: string, plural = `${singular}s`) {
		if (count === 0) return null;
		return `${count} ${count === 1 ? singular : plural}`;
	}

	function protocolMultisigCount(protocol: SelectableProtocol) {
		return new Set((protocol.related_multisigs ?? []).map((link) => link.multisig)).size;
	}

	function protocolGovernanceCount(protocol: SelectableProtocol) {
		return uniqueProtocolGovernanceCount(protocol);
	}

	function linkedItemSummary(protocol: SelectableProtocol) {
		return [
			pluralCount(protocolMultisigCount(protocol), 'multisig'),
			pluralCount(protocolGovernanceCount(protocol), 'realm'),
			pluralCount(protocol.sol_domains.length, '.sol name', '.sol names')
		]
			.filter(Boolean)
			.join(' · ');
	}

	function protocolItemSummary(protocol: SelectableProtocol) {
		return [
			pluralCount(protocol.programs.length, 'program'),
			pluralCount(protocolMultisigCount(protocol), 'multisig'),
			pluralCount(protocolGovernanceCount(protocol), 'realm'),
			pluralCount(protocol.sol_domains.length, '.sol name', '.sol names'),
			pluralCount(protocol.amount_triggers.length, 'rule')
		]
			.filter(Boolean)
			.join(', ');
	}

	function protocolPickerSummary(protocol: SelectableProtocol) {
		return [
			pluralCount(protocol.programs.length, 'program'),
			pluralCount(protocolSourceCount(protocol), 'source')
		]
			.filter(Boolean)
			.join(' · ');
	}

	function onPageKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			closeProtocolPicker();
			closeClearConfigConfirm();
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

	function paymentLinks() {
		return [...(data.billing?.payment_links ?? [])].sort((left, right) => right.created_at - left.created_at);
	}

	function activePaymentLinks() {
		const now = Math.floor(Date.now() / 1000);
		return paymentLinks()
			.filter((link) => link.status === 'pending' && link.expires_at > now)
			.slice(0, PAYMENT_LINK_PREVIEW_LIMIT);
	}

	function visiblePaymentLinks() {
		const pending = activePaymentLinks();
		return pending.length > 0 ? pending : paymentLinks().slice(0, PAYMENT_LINK_PREVIEW_LIMIT);
	}

	function primaryPaymentLink() {
		return activePaymentLinks()[0] ?? null;
	}

	function paymentLinkToken(link: PaymentLink) {
		return link.spl_token ? pricingToken() : 'SOL';
	}

	function paymentLinkAmountLine(link: PaymentLink) {
		return `${link.amount} ${paymentLinkToken(link)}`;
	}

	function paymentLinkStatusLabel(link: PaymentLink) {
		if (link.status === 'pending' && link.expires_at <= Math.floor(Date.now() / 1000)) {
			return 'expired';
		}
		return link.status.replaceAll('_', ' ');
	}

	function paymentLinkDetail(link: PaymentLink) {
		const status = paymentLinkStatusLabel(link);
		if (status === 'paid' && link.paid_at) return `Paid ${formatTimestamp(link.paid_at)}`;
		if (status === 'expired') return `Expired ${formatTimestamp(link.expires_at)}`;
		return `${status} · expires ${formatTimestamp(link.expires_at)}`;
	}

	function paymentLinkCheckoutToken(link: PaymentLink) {
		try {
			const pathname = new URL(link.public_url).pathname;
			return pathname.split('/pay/')[1]?.split('/')[0] ?? '';
		} catch {
			return link.public_url.split('/pay/')[1]?.split('/')[0] ?? '';
		}
	}

	function billingStatusLine() {
		if (data.billing?.active) return 'Live alerts active';
		if (activePaymentLinks().length > 0) return 'Payment link ready';
		return 'Checkout not opened yet';
	}

	function billingPlanLine() {
		const month = billingAmountLine('month');
		const year = billingAmountLine('year');
		if (month && year) return `${month} monthly · ${year} yearly`;
		return month ?? year ?? 'Configured at checkout';
	}

	function saveButtonLabel() {
		if (canSaveClearedConfig) return 'Save cleared config';
		if (!hasDeliveryModeSelected) return 'Choose delivery';
		if (data.billing?.active) return 'Save configuration';
		if (needsLiveCheckout) return 'Save and open checkout';
		if (dailyDeliveryEnabled) return 'Save daily summary';
		return 'Save setup';
	}

	function saveOutcomeLabel() {
		if (canSaveClearedConfig) return 'Save to remove all current watch rules';
		if (!hasDeliveryModeSelected) return 'Choose how this chat should receive alerts';
		if (data.billing?.active) return 'Changes apply to the active watch';
		if (needsLiveCheckout) return 'Save the setup, then activate live alerts in checkout';
		return 'Daily summaries save immediately';
	}

	function deliveryModeLabel(mode: TelegramDeliveryMode) {
		return mode === 'daily_summary' ? 'Daily summary' : 'Live alerts';
	}

	function deliveryModeHint(mode: TelegramDeliveryMode) {
		if (mode === 'daily_summary') return 'One Telegram digest per day';
		return data.billing?.active ? 'Send each alert as it lands' : 'Saved now, activates after checkout';
	}

	function deliverySummaryLabel() {
		if (dailyDeliveryEnabled && liveDeliveryEnabled) return 'Daily + live';
		if (liveDeliveryEnabled) return 'Live alerts';
		if (dailyDeliveryEnabled) return 'Daily summary';
		return 'None';
	}

	function activationStatusLabel() {
		if (canSaveClearedConfig) return 'Ready to clear';
		if (!hasDeliveryModeSelected) return 'Delivery missing';
		if (data.billing?.active) return 'Ready to save';
		if (needsLiveCheckout) return 'Checkout required';
		return 'Ready to save';
	}

	function activationStatusHint() {
		if (canSaveClearedConfig) return 'This will remove the saved watch rules for this chat.';
		if (!hasDeliveryModeSelected) return 'Select daily summaries, live alerts, or both.';
		if (data.billing?.active) return 'Live alerts are already active for this Telegram user.';
		if (needsLiveCheckout) return 'Daily summaries save now. Live alerts activate after payment.';
		return 'No checkout needed for daily summaries.';
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

	let walletAddressesValue = $state(initialWalletAddressList.join('\n'));
	let solDomainsValue = $state(initialSolDomainList.join('\n'));
	let lastSyncedLoadedWalletConfigKey = $state(
		`${initialWalletAddressList.join('|')}::${initialSolDomainList.join('|')}`
	);
	let watchTargetInput = $state('');
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

	function normalizedSolDomain(value: string) {
		const trimmed = value.trim().toLowerCase();
		if (!trimmed) return null;
		return SOL_DOMAIN_PATTERN.test(trimmed) ? trimmed : null;
	}

	function parseSolDomainLines(raw: string) {
		const tokens = raw
			.split(/[\n,]+/)
			.map((value) => value.trim())
			.filter(Boolean);
		const seen = new Set<string>();
		const domains: string[] = [];
		const invalid: string[] = [];

		for (const token of tokens) {
			const normalized = normalizedSolDomain(token);
			if (!normalized) {
				invalid.push(token);
				continue;
			}
			if (seen.has(normalized)) continue;
			seen.add(normalized);
			domains.push(normalized);
		}

		return { domains, invalid };
	}

	function setWalletAddressList(addresses: string[]) {
		walletAddressesValue = addresses.join('\n');
	}

	function setSolDomainList(domains: string[]) {
		solDomainsValue = domains.join('\n');
	}

	function mergeLoadedWalletAddresses() {
		const { addresses, invalid } = parseWalletAddressLines(walletAddressesValue);
		if (invalid.length > 0) return;
		const merged = [...addresses];
		for (const address of loadedWalletAddressList) {
			if (!merged.includes(address)) {
				merged.push(address);
			}
		}
		if (merged.length !== addresses.length) {
			setWalletAddressList(merged);
		}
	}

	function mergeLoadedSolDomains() {
		const { domains, invalid } = parseSolDomainLines(solDomainsValue);
		if (invalid.length > 0) return;
		const merged = [...domains];
		for (const domain of loadedSolDomainList) {
			if (!merged.includes(domain)) {
				merged.push(domain);
			}
		}
		if (merged.length !== domains.length) {
			setSolDomainList(merged);
		}
	}

	$effect(() => {
		const key = loadedWalletConfigKey;
		if (key === lastSyncedLoadedWalletConfigKey) return;
		lastSyncedLoadedWalletConfigKey = key;
		if (clearAllConfirmed) return;
		mergeLoadedWalletAddresses();
		mergeLoadedSolDomains();
	});

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
		clearAllConfirmed = false;

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

	function addSolDomain(rawDomain: string) {
		const normalized = normalizedSolDomain(rawDomain);
		if (!normalized) {
			throw new Error('Enter a wallet address or a .sol name.');
		}

		const { domains, invalid } = parseSolDomainLines(solDomainsValue);
		if (invalid.length > 0) {
			throw new Error(`Fix ${invalid.length} invalid .sol entries first.`);
		}

		if (!domains.includes(normalized)) {
			domains.push(normalized);
			setSolDomainList(domains);
		}
		clearAllConfirmed = false;
	}

	function addWatchTarget(rawValue: string) {
		const trimmed = rawValue.trim();
		if (!trimmed) {
			throw new Error('Enter a wallet address or a .sol name.');
		}

		const walletAddress = normalizedWalletAddress(trimmed);
		if (walletAddress) {
			addWalletAddress(walletAddress);
			return `Wallet ${shortAddress(walletAddress)} added.`;
		}

		const solDomain = normalizedSolDomain(trimmed);
		if (solDomain) {
			addSolDomain(solDomain);
			return `${solDomain} added.`;
		}

		throw new Error('Enter a valid Solana wallet address or a .sol name.');
	}

	function addWatchTargetFromInput() {
		try {
			const message = addWatchTarget(watchTargetInput);
			watchTargetInput = '';
			walletImportError = null;
			walletImportMessage = message;
		} catch (error) {
			walletImportMessage = null;
			walletImportError = error instanceof Error ? error.message : 'Unable to add this target.';
		}
	}

	function onWatchTargetKeydown(event: KeyboardEvent) {
		if (event.key !== 'Enter') return;
		event.preventDefault();
		addWatchTargetFromInput();
	}

	function removeWalletAddress(address: string) {
		const nextAddresses = walletPreview.addresses.filter((candidate) => candidate !== address);
		setWalletAddressList(nextAddresses);
		const remainingNames = { ...walletNamesByAddress };
		const remainingBundles = { ...walletBundleIdsByAddress };
		delete remainingNames[address];
		delete remainingBundles[address];
		walletNamesByAddress = remainingNames;
		walletBundleIdsByAddress = remainingBundles;
	}

	function removeSolDomain(domain: string) {
		setSolDomainList(solDomainPreview.domains.filter((candidate) => candidate !== domain));
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
	const solDomainPreview = $derived(parseSolDomainLines(solDomainsValue));
	const watchTargetCount = $derived(walletPreview.addresses.length + solDomainPreview.domains.length);
	const selectedConfigCount = $derived(selectedProtocols.length + watchTargetCount);
	const hasDeliveryModeSelected = $derived(dailyDeliveryEnabled || liveDeliveryEnabled);
	const needsLiveCheckout = $derived(!data.billing?.active && liveDeliveryEnabled);
	const canSaveClearedConfig = $derived(
		clearAllConfirmed && selectedConfigCount === 0 && currentSubscriptionCount > 0
	);
	const canSaveConfig = $derived(
		(selectedConfigCount > 0 && hasDeliveryModeSelected) || canSaveClearedConfig
	);

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
		<section class="setup-intro">
			<div>
				<h1>Set up FireWatch for {chatLabel()}</h1>
				<p>{userLabel()} · remembered until {formatTimestamp(data.session.expires_at)}</p>
			</div>
			<div class="setup-status">
				<strong>{data.billing?.active ? 'Live alerts active' : 'Draft setup'}</strong>
				<span>{data.billing?.active ? 'Changes apply on save' : 'Payment activates live alerts'}</span>
			</div>
		</section>

		<form id="config-form" class="config-form" method="POST" action="?/save_config">
			{#if clearAllConfirmed}
				<input type="hidden" name="clear_config" value="true" />
			{/if}
			{#each data.subscriptions ?? [] as subscription (subscription.id)}
				<input type="hidden" name="unsubscribe_ids" value={subscription.id} />
			{/each}
			<div class="panel mobile-payment-strip">
				<div>
					<strong>Payment</strong>
					<span>{billingStatusLine()}</span>
				</div>
				<p>{data.billing?.active ? 'Live alerts are active.' : billingPlanLine()}</p>
				{#if primaryPaymentLink()}
					{#each activePaymentLinks().slice(0, 1) as mobilePaymentLink (mobilePaymentLink.id)}
						<a href={resolve('/pay/[token]', { token: paymentLinkCheckoutToken(mobilePaymentLink) })}>
							Pay {paymentLinkAmountLine(mobilePaymentLink)}
						</a>
					{/each}
				{:else if !data.billing?.active}
					<button
						class="secondary-button"
						type="submit"
						formaction="?/create_checkout"
						formmethod="POST"
					>
						Create checkout link
					</button>
				{/if}
			</div>
			<div class="config-layout">
				<div class="config-main">
					<section class="panel field-panel">
						<div class="section-head">
							<h2>Connect or add wallets</h2>
							<p>Wallet watches drive live protocol discovery. Add a public key, connect your wallet, or include a .sol name.</p>
						</div>
						<input type="hidden" name="wallet_addresses" value={walletAddressesValue} />
						<input type="hidden" name="sol_domains" value={solDomainsValue} />
						<div class="wallet-connect-card">
							<div>
								<strong>Connect wallet</strong>
								<span>
									{#if wallet.connected && wallet.publicKey}
										{shortAddress(wallet.publicKey)}
									{:else}
										Connect, then add the connected wallet to the watch.
									{/if}
								</span>
							</div>
							<div class="wallet-connect-actions">
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
						</div>
						<div class="target-add-row primary-target-row">
							<input
								bind:value={watchTargetInput}
								onkeydown={onWatchTargetKeydown}
								placeholder="Wallet public key or ferno.sol"
								type="text"
							/>
							<button type="button" onclick={addWatchTargetFromInput}>Add target</button>
						</div>

						{#if walletImportError}
							<p class="wallet-feedback error">{walletImportError}</p>
						{:else if walletImportMessage}
							<p class="wallet-feedback success">{walletImportMessage}</p>
						{/if}

						<div class="target-list">
							{#each walletPreview.addresses as address (address)}
								<div class="target-row">
									<div>
										<strong>Wallet</strong>
										<code>{shortAddress(address)}</code>
										{#if walletNamesByAddress[address]}
											<span>{walletNamesByAddress[address]}</span>
										{/if}
									</div>
									<button class="secondary-button" type="button" onclick={() => removeWalletAddress(address)}>
										Remove
									</button>
								</div>
							{/each}
							{#each solDomainPreview.domains as domain (domain)}
								<div class="target-row">
									<div>
										<strong>.sol</strong>
										<span>{domain}</span>
									</div>
									<button class="secondary-button" type="button" onclick={() => removeSolDomain(domain)}>
										Remove
									</button>
								</div>
							{/each}
							{#if walletPreview.addresses.length === 0 && solDomainPreview.domains.length === 0}
								<p>No wallet-specific targets yet.</p>
							{/if}
						</div>

						<div class="wallet-tools">
							<button class="secondary-button" type="button" onclick={openWalletImport}>Import wallet JSON</button>
							<button class="secondary-button" type="button" onclick={exportWalletAddressbook}>Export wallet JSON</button>
							<input
								bind:this={walletImportInput}
								class="wallet-import-input"
								type="file"
								accept=".json,application/json"
								onchange={onWalletImportChange}
							/>
						</div>
					</section>

					<section class="panel protocol-panel">
						<div class="section-head protocol-head">
							<div>
								<h2>Protocols for this user</h2>
								<p>Protocol rules are learned from wallet activity first. Manual additions stay available for known coverage.</p>
							</div>
							<div class="protocol-head-actions">
								<button
									class="secondary-button"
									type="button"
									disabled={selectedProtocols.length === selectableProtocolList.length}
									onclick={addAllProtocols}
								>
									Add all
								</button>
								<button class="secondary-button" type="button" onclick={openProtocolPicker}>
									Add protocol
								</button>
								<button class="secondary-button danger-button" type="button" onclick={openClearConfigConfirm}>
									Clear all
								</button>
							</div>
						</div>
						<div class="discovery-panel">
							<div class="discovery-head">
								<div>
									<strong>Auto-discovery</strong>
									<span>{watchTargetCount === 0 ? 'Add wallet targets first' : `${watchTargetCount} wallet target${watchTargetCount === 1 ? '' : 's'}`}</span>
								</div>
								<small>
									Use wallet activity to recommend protocol rules. Auto-discovered rules appear in the same list as manual rules.
								</small>
							</div>
							<div class="discovery-options">
								<div class="discovery-action" class:disabled={watchTargetCount === 0}>
									<span>
										<strong>Scan wallet history</strong>
										<small>Find protocols touched by recent wallet activity.</small>
									</span>
									<button
										class="secondary-button"
										type="button"
										disabled={watchTargetCount === 0 || walletScanState === 'scanning'}
										onclick={scanWalletHistory}
									>
										{walletScanState === 'scanning' ? 'Scanning...' : 'Scan now'}
									</button>
								</div>
								<div class="discovery-action passive" class:disabled={watchTargetCount === 0}>
									<span>
										<strong>Live wallet follow</strong>
										<small>Saved wallet watches auto-add new protocol rules from indexed activity.</small>
									</span>
								</div>
							</div>
							<div class="auto-rules-summary">
								<strong>Discovered rules</strong>
								<span>
									{#if autoDiscoveredProtocols.length > 0}
										{autoDiscoveredProtocols.length} in watchlist
										{#if autoDiscoveredProtocols.length <= 3}
											· {autoDiscoveredProtocols.map((protocol) => protocol.name).join(', ')}
										{/if}
									{:else}
										Scan a wallet or wait for live wallet follow to add recommendations.
									{/if}
								</span>
							</div>
						</div>
						<div class="user-protocols-panel">
							<div class="user-protocols-head">
								<div>
									<strong>User protocol list</strong>
									<span>
										{selectedProtocols.length}
										{selectedProtocols.length === 1 ? 'protocol' : 'protocols'}
										{#if autoDiscoveredProtocols.length > 0}
											· {autoDiscoveredProtocols.length} learned
										{/if}
									</span>
								</div>
								<button class="secondary-button" type="button" onclick={openProtocolPicker}>
									Add manually
								</button>
							</div>
							{#if userProtocolPreview.length > 0}
								<div class="user-protocol-chip-list">
									{#each userProtocolPreview as protocol (protocol.slug)}
										<button
											class="user-protocol-chip"
											type="button"
											onclick={() => openProtocolDetails(protocol.slug)}
										>
											<ProtocolIdentity slug={protocol.slug} label={protocol.name} />
											<small>{protocolSourceLabel(protocol.slug)}</small>
										</button>
									{/each}
									{#if hiddenUserProtocolCount > 0}
										<span class="user-protocol-more">+{hiddenUserProtocolCount} more</span>
									{/if}
								</div>
							{:else}
								<p>
									Add a wallet, then scan recent activity or let live wallet follow learn protocols from new indexed transactions.
								</p>
							{/if}
						</div>
						<div class="custom-rules-card active">
							<div>
								<strong>Manual coverage</strong>
								<span>{selectedCustomProtocolSlugs.length} selected</span>
								<small>Use curated coverage or hand-pick protocols when the wallet has not touched them yet.</small>
							</div>
							<div class="custom-rule-actions">
								<button class="secondary-button" type="button" onclick={useCuratedPreset}>
									Use curated preset
								</button>
								<button
									class="secondary-button"
									type="button"
									disabled={selectedProtocols.length === selectableProtocolList.length}
									onclick={addAllProtocols}
								>
									Add all protocols
								</button>
								<button class="secondary-button" type="button" onclick={openProtocolPicker}>
									Add protocol
								</button>
								<button class="secondary-button danger-button" type="button" onclick={openClearConfigConfirm}>
									Clear all
								</button>
							</div>
						</div>
						{#if walletScanError}
							<p class="wallet-feedback error">{walletScanError}</p>
						{/if}
						{#if autoDiscoveryMessage}
							<p class="wallet-feedback success">{autoDiscoveryMessage}</p>
						{/if}
						{#if protocolRuleMessage}
							<p class="wallet-feedback success">{protocolRuleMessage}</p>
						{/if}
						{#each protocolSlugsForSave as slug (slug)}
							{@const level = selectedProtocolLevel(slug)}
							<input type="hidden" name="protocol_slugs" value={slug} />
							<input type="hidden" name="protocol_levels" value={`${slug}:${protocolLevel(level)}`} />
						{/each}
						{#each protocolProgramSlugsForSave as slug (slug)}
							<input type="hidden" name="protocol_slugs" value={slug} />
							<input type="hidden" name="protocol_levels" value={`${slug}:${protocolLevel(selectedProtocolLevel(slug))}`} />
						{/each}
						{#each protocolProgramIdsForSave as entry (entry)}
							<input type="hidden" name="protocol_program_ids" value={entry} />
						{/each}
						<div class="protocol-list">
							{#if selectedProtocols.length > 0}
								<div class="watchlist-tools">
									<label class="watchlist-search">
										<span>Find in watchlist</span>
										<input
											bind:value={selectedProtocolSearch}
											placeholder="Protocol, program, or address"
											type="search"
										/>
									</label>
									<div class="source-filter-row" role="group" aria-label="Watchlist source">
										{#each selectedProtocolSourceFilters as sourceFilter (sourceFilter)}
											<button
												class="secondary-button source-filter-button"
												class:active={selectedProtocolSourceFilter === sourceFilter}
												type="button"
												aria-pressed={selectedProtocolSourceFilter === sourceFilter}
												onclick={() => (selectedProtocolSourceFilter = sourceFilter)}
											>
												{protocolSourceFilterLabel(sourceFilter)}
												<span>{protocolSourceFilterCount(sourceFilter)}</span>
											</button>
										{/each}
									</div>
								</div>
							{/if}
							{#each displayedSelectedProtocols as protocol (protocol.slug)}
								<div class:open={activeProtocolSlug === protocol.slug} class="protocol-entry">
									<div class="protocol-row">
										<button
											class="protocol-row-main"
											type="button"
											aria-expanded={activeProtocolSlug === protocol.slug}
											onclick={() => openProtocolDetails(protocol.slug)}
										>
											<span>
												<ProtocolIdentity slug={protocol.slug} label={protocol.name} />
												<small>{selectedProtocolItemSummary(protocol)}</small>
											</span>
											<span class="protocol-row-meta">
												<small>{protocolSourceLabel(protocol.slug)}</small>
												<strong>{protocolLevelLabel(selectedProtocolLevel(protocol.slug))}</strong>
											</span>
											<span class="configure-label">Configure</span>
										</button>
										<button
											class="secondary-button row-remove-button"
											type="button"
											onclick={() => removeCustomProtocol(protocol.slug)}
										>
											Remove
										</button>
									</div>
									{#if activeProtocolSlug === protocol.slug}
										<div class="protocol-detail">
											<div class="protocol-detail-controls">
												<label class="severity-control">
													<span>Severity</span>
													<select
														value={selectedProtocolLevel(protocol.slug)}
														aria-label={`Severity for ${protocol.name}`}
														onchange={(event) => onCustomProtocolLevelChange(protocol.slug, event)}
													>
														<option value="normal">Normal</option>
														<option value="high">High</option>
														<option value="critical">Critical</option>
													</select>
												</label>
												<p class="protocol-detail-note">
													Refine what this protocol rule watches. Multisigs, accounts, and metric triggers stay attached to the protocol.
												</p>
											</div>

											{#if protocol.programs.length > 0}
												<section class="detail-section">
													<div class="detail-section-head">
														<strong>Programs</strong>
														<span>{selectedProgramIdsForProtocol(protocol).length} of {protocol.programs.length}</span>
													</div>
													<div class="detail-items">
														{#each protocol.programs as program (program.program_id)}
															<label class="detail-item">
																<input
																	type="checkbox"
																	checked={isProtocolProgramSelected(protocol, program.program_id)}
																	onchange={() => toggleProtocolProgram(protocol, program.program_id)}
																/>
																<span>
																	<strong>{program.label}</strong>
																	<code>{shortAddress(program.program_id)}</code>
																</span>
																<em>Program</em>
															</label>
														{/each}
													</div>
												</section>
											{/if}

											{#if (protocol.related_multisigs ?? []).length > 0 || (protocol.related_governances ?? []).length > 0 || protocol.sol_domains.length > 0}
												<section class="detail-section">
													<div class="detail-section-head">
														<strong>Linked items</strong>
														<span>{linkedItemSummary(protocol)}</span>
													</div>
													<div class="detail-items">
														{#each protocol.related_multisigs ?? [] as link (`${link.multisig}:${link.program_id}`)}
															<div class="detail-item static">
																<span>
																	<strong>{link.program_label || 'Squads vault'}</strong>
																	<code>{shortAddress(link.multisig)}</code>
																</span>
																<em>Multisig</em>
															</div>
														{/each}
														{#each protocol.related_governances ?? [] as link (`${link.realm}:${link.governance}:${link.governed_account}`)}
															<div class="detail-item static">
																<span>
																	<strong>{link.realm_name || 'Realms governance'}</strong>
																	<code>{shortAddress(link.realm || link.governance || link.governed_account)}</code>
																</span>
																<em>Realm</em>
															</div>
														{/each}
														{#each protocol.sol_domains as domain (domain)}
															<div class="detail-item static">
																<span>
																	<strong>{domain}</strong>
																	<code>.sol name</code>
																</span>
																<em>Account</em>
															</div>
														{/each}
													</div>
												</section>
											{/if}

											{#if protocol.amount_triggers.length > 0}
												<section class="detail-section">
													<div class="detail-section-head">
														<strong>Metric triggers</strong>
														<span>{protocol.amount_triggers.length}</span>
													</div>
													<div class="metric-rule-list">
														{#each protocol.amount_triggers as trigger (trigger.label)}
															{@const triggerKey = metricTriggerKey(protocol, trigger.label)}
															{@const presets = metricPresetsForTrigger(protocol, trigger.label)}
															{@const selectedPreset = selectedMetricPresetForTrigger(protocol, trigger.label)}
															<div class="metric-rule-item">
																<div class="metric-rule-head">
																	<span>
																		<strong>{trigger.label}</strong>
																		<code>{trigger.description}</code>
																	</span>
																	<em>{selectedPreset && hasSavedMetricRule(selectedPreset) ? 'Active' : 'Rule'}</em>
																</div>
																{#if selectedPreset}
																	<input
																		type="hidden"
																		name={metricFieldName(triggerKey, 'label')}
																		value={`${protocol.name} ${trigger.label}`}
																	/>
																	<input
																		type="hidden"
																		name={metricFieldName(triggerKey, 'protocol_slug')}
																		value={selectedPreset.protocol_slug}
																	/>
																	<input
																		type="hidden"
																		name={metricFieldName(triggerKey, 'instruction_name')}
																		value={selectedPreset.instruction_name}
																	/>
																	<input
																		type="hidden"
																		name={metricFieldName(triggerKey, 'metric_key')}
																		value={selectedPreset.metric_key}
																	/>
																	<input
																		type="hidden"
																		name={metricFieldName(triggerKey, 'subject_address')}
																		value={selectedPreset.subject_address ?? ''}
																	/>

																	<div class="metric-rule-controls">
																		<label>
																			<span>Observed metric</span>
																			<select
																				name={metricFieldName(triggerKey, 'preset')}
																				value={metricPresetKey(selectedPreset)}
																				onchange={(event) => onMetricPresetChange(triggerKey, event)}
																			>
																				{#each presets as preset (metricPresetKey(preset))}
																					<option value={metricPresetKey(preset)}>
																						{readableInstructionName(preset.instruction_name)} · {preset.metric_label} · {metricSubjectLabel(preset)}
																					</option>
																				{/each}
																			</select>
																		</label>
																		<div class="metric-bound-row">
																			<label>
																				<span>Min</span>
																				<input
																					name={metricFieldName(triggerKey, 'min_value')}
																					type="number"
																					step="any"
																					value={selectedPreset.suggested_min_value ?? ''}
																				/>
																			</label>
																			<label>
																				<span>Max</span>
																				<input
																					name={metricFieldName(triggerKey, 'max_value')}
																					type="number"
																					step="any"
																					value={selectedPreset.suggested_max_value ?? ''}
																				/>
																			</label>
																		</div>
																	</div>
																	<div class="metric-rule-footer">
																		<span>
																			Latest {formatMetricValue(selectedPreset.latest_value)} · slot {selectedPreset.latest_slot}
																		</span>
																		<button
																			class="secondary-button"
																			type="submit"
																			formaction="?/create_metric_alert_rule"
																			formmethod="POST"
																			name="metric_rule_key"
																			value={triggerKey}
																		>
																			Save trigger
																		</button>
																	</div>
																{:else}
																	<p class="metric-rule-empty">
																		No observed metric preset yet. This trigger unlocks after FireWatch sees the matching instruction.
																	</p>
																{/if}
															</div>
														{/each}
													</div>
												</section>
											{/if}
										</div>
									{/if}
								</div>
							{:else}
								<p class="empty-payments">
									{selectedProtocols.length === 0
										? 'No protocol rules selected yet.'
										: 'No selected protocols match this view.'}
								</p>
							{/each}
							{#if filteredSelectedProtocols.length > SELECTED_PROTOCOL_PREVIEW_LIMIT}
								<button
									class="secondary-button list-toggle-button"
									type="button"
									onclick={() => (selectedProtocolListExpanded = !selectedProtocolListExpanded)}
								>
									{selectedProtocolListExpanded
										? 'Show fewer protocols'
										: `Show ${hiddenSelectedProtocolCount} more protocols`}
								</button>
							{/if}
						</div>
					</section>
				</div>

				<aside class="config-side">
					<section id="billing" class="panel checkout-panel">
						<div class="section-head">
							<h2>Review and activate</h2>
							<p>{saveOutcomeLabel()}</p>
						</div>

						<div
							class:checkout={needsLiveCheckout}
							class:paid={data.billing?.active}
							class="activation-status"
						>
							<strong>{activationStatusLabel()}</strong>
							<span>{activationStatusHint()}</span>
						</div>

						<div class="save-summary">
							<div>
								<span>Wallets and .sol</span>
								<strong>{watchTargetCount}</strong>
							</div>
							<div>
								<span>Protocols</span>
								<strong>{selectedProtocols.length}</strong>
							</div>
							<div>
								<span>Delivery</span>
								<strong>{deliverySummaryLabel()}</strong>
							</div>
						</div>

						<div class="review-group payment-group">
							<div class="review-group-head">
								<strong>Payment</strong>
								<span>{data.billing?.active ? 'Current access' : 'Checkout'}</span>
							</div>
							<div class="payment-summary">
								<div>
									<span>Status</span>
									<strong>{billingStatusLine()}</strong>
								</div>
								{#if data.billing?.active}
									<div>
										<span>Credit</span>
										<strong>{data.billing.balance_months} mo</strong>
									</div>
									{#if data.billing.next_payment_due_at}
										<div>
											<span>Next due</span>
											<strong>{formatTimestamp(data.billing.next_payment_due_at)}</strong>
										</div>
									{/if}
								{:else}
									<div>
										<span>Plan</span>
										<strong>{billingPlanLine()}</strong>
									</div>
								{/if}
							</div>
							{#if !data.billing?.active}
								<div class="billing-row compact">
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
							{#if visiblePaymentLinks().length > 0}
								<div class="payment-link-list">
									{#each visiblePaymentLinks() as link (link.id)}
										<div class="payment-link-row">
											<div>
												<strong>{paymentLinkAmountLine(link)}</strong>
												<span>{paymentLinkDetail(link)}</span>
											</div>
											<a href={resolve('/pay/[token]', { token: paymentLinkCheckoutToken(link) })}>
												{link.status === 'pending' ? 'Pay' : 'Open'}
											</a>
										</div>
									{/each}
								</div>
							{/if}
							{#if !data.billing?.active}
								<div class="payment-action-row">
									<button
										class="secondary-button payment-create-button"
										type="submit"
										formaction="?/create_checkout"
										formmethod="POST"
									>
										Create checkout link
									</button>
									<span>Opens a Solana Pay checkout for the selected billing period.</span>
								</div>
							{/if}
						</div>

						<div class="review-group">
							<div class="review-group-head">
								<strong>Delivery</strong>
								<span>Choose one or both</span>
							</div>
							<div class="delivery-row" role="group" aria-label="Delivery modes">
								<label class="choice-pill">
									<input
										bind:checked={dailyDeliveryEnabled}
										type="checkbox"
										name="delivery_modes"
										value="daily_summary"
									/>
									<span>{deliveryModeLabel('daily_summary')}</span>
									<small>{deliveryModeHint('daily_summary')}</small>
								</label>
								<label class="choice-pill">
									<input
										bind:checked={liveDeliveryEnabled}
										type="checkbox"
										name="delivery_modes"
										value="live"
									/>
									<span>{deliveryModeLabel('live')}</span>
									<small>{deliveryModeHint('live')}</small>
								</label>
							</div>
						</div>
						{#if data.billing?.active && liveDeliveryEnabled}
							<p class="review-note success">Live alerts are active for this Telegram user.</p>
						{:else if dailyDeliveryEnabled && !liveDeliveryEnabled}
							<p class="review-note">Daily summaries can be saved without checkout.</p>
						{/if}
						{#if !canSaveConfig}
							<p class="wallet-feedback error">
								{selectedConfigCount === 0
									? 'Add at least one wallet, .sol name, or protocol before saving.'
									: 'Choose at least one delivery option.'}
							</p>
						{/if}
						<button type="submit" disabled={!canSaveConfig}>{saveButtonLabel()}</button>
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
							<p>{selectedProtocolSlugs.length} selected · {protocolPickerOptions.length} available</p>
						</div>
						<button class="secondary-button" type="button" onclick={closeProtocolPicker}>
							Back
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
						{#each protocolPickerOptions as protocol (protocol.slug)}
							<div class="program-picker-row">
								<label class="program-picker-main">
									<input
										checked={false}
										type="checkbox"
										onchange={() => toggleCustomProtocol(protocol.slug)}
									/>
									<span>
										<ProtocolIdentity slug={protocol.slug} label={protocol.name} size="sm" />
										<small>
											{protocolPickerSummary(protocol)}
										</small>
									</span>
								</label>
								<button
									class="secondary-button program-add-button"
									type="button"
									onclick={() => toggleCustomProtocol(protocol.slug)}
								>
									Add
								</button>
							</div>
						{:else}
							<p class="empty-payments">No protocols match this search.</p>
						{/each}
					</div>
				</div>
			</div>
		{/if}

		{#if clearConfigConfirmOpen}
			<div class="modal-backdrop" role="presentation" onclick={closeClearConfigConfirm}>
				<div
					class="confirm-modal"
					aria-label="Clear current configuration"
					aria-modal="true"
					role="dialog"
					tabindex="-1"
					onclick={(event) => event.stopPropagation()}
					onkeydown={(event) => event.stopPropagation()}
				>
					<header class="modal-head">
						<div>
							<h2>Clear all config?</h2>
							<p>
								This removes every selected protocol, wallet, and .sol target from the draft.
								{currentSubscriptionCount > 0
									? 'Save the cleared config afterward to remove the current watch rules.'
									: 'There are no saved watch rules to remove yet.'}
							</p>
						</div>
					</header>
					<div class="confirm-summary">
						<div>
							<span>Protocols</span>
							<strong>{selectedProtocols.length}</strong>
						</div>
						<div>
							<span>Wallets and .sol</span>
							<strong>{watchTargetCount}</strong>
						</div>
						<div>
							<span>Saved rules</span>
							<strong>{currentSubscriptionCount}</strong>
						</div>
					</div>
					<div class="confirm-actions">
						<button class="secondary-button" type="button" onclick={closeClearConfigConfirm}>
							Cancel
						</button>
						<button class="danger-button" type="button" onclick={clearAllConfig}>
							Clear all
						</button>
					</div>
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

	.setup-intro,
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

	.mobile-payment-strip {
		display: none;
	}

	.setup-intro {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		gap: 1rem;
		padding: 0.35rem 0 0.15rem;
	}

	.setup-intro div,
	.setup-status {
		display: grid;
		gap: 0.16rem;
		min-width: 0;
	}

	.setup-intro h1 {
		margin: 0;
		font-size: clamp(1.28rem, 2vw, 1.75rem);
		line-height: 1.05;
	}

	.setup-intro p,
	.setup-status span {
		margin: 0;
		color: #5f6c79;
		font-size: 0.82rem;
		font-weight: 700;
	}

	.setup-status {
		justify-items: end;
		text-align: right;
	}

	.setup-status strong {
		color: #32704c;
		font-size: 0.88rem;
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

	@media (min-width: 841px) {
		.config-side {
			position: sticky;
			top: calc(var(--fw-header-height, 0px) + 0.85rem);
			max-height: calc(100vh - var(--fw-header-height, 0px) - 1.2rem);
			overflow: auto;
			overscroll-behavior: contain;
		}
	}

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

	.program-picker-main span,
	.modal-head div {
		display: grid;
		gap: 0.16rem;
		min-width: 0;
	}

	.protocol-panel {
		display: grid;
		gap: 0.85rem;
	}

	.protocol-head {
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: start;
	}

	.protocol-head-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.45rem;
		justify-content: end;
	}

	.discovery-panel {
		display: grid;
		gap: 0.65rem;
		padding: 0.72rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		background: #fbfaf8;
	}

	.discovery-head {
		display: grid;
		grid-template-columns: minmax(0, 0.85fr) minmax(0, 1.15fr);
		gap: 0.75rem;
		align-items: start;
	}

	.discovery-head div,
	.auto-rules-summary {
		display: grid;
		gap: 0.14rem;
		min-width: 0;
	}

	.discovery-head span,
	.auto-rules-summary span {
		color: #5f6c79;
		font-size: 0.78rem;
		font-weight: 700;
	}

	.discovery-options {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 0.55rem;
	}

	.discovery-action {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.65rem;
		padding: 0.62rem 0.7rem;
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 8px;
		background: #fff;
	}

	.discovery-action.disabled {
		opacity: 0.64;
	}

	.discovery-action.passive {
		grid-template-columns: minmax(0, 1fr);
	}

	.discovery-action span {
		display: grid;
		gap: 0.08rem;
		min-width: 0;
	}

	.discovery-action small {
		color: #5f6c79;
		font-size: 0.78rem;
		font-weight: 700;
		line-height: 1.45;
	}

	.auto-rules-summary {
		padding-top: 0.58rem;
		border-top: 1px solid rgba(23, 32, 42, 0.08);
	}

	.user-protocols-panel {
		display: grid;
		gap: 0.65rem;
		padding: 0.72rem;
		border: 1px solid rgba(50, 112, 76, 0.22);
		border-radius: 8px;
		background: #f7fbf8;
	}

	.user-protocols-head {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.75rem;
	}

	.user-protocols-head > div {
		display: grid;
		gap: 0.14rem;
		min-width: 0;
	}

	.user-protocols-head span,
	.user-protocols-panel p,
	.user-protocol-chip small,
	.user-protocol-more {
		color: #5f6c79;
		font-size: 0.78rem;
		font-weight: 700;
	}

	.user-protocols-panel p {
		margin: 0;
		line-height: 1.45;
	}

	.user-protocol-chip-list {
		display: flex;
		flex-wrap: wrap;
		gap: 0.45rem;
		align-items: center;
	}

	.user-protocol-chip {
		display: inline-grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.5rem;
		max-width: 100%;
		min-height: 2.35rem;
		padding: 0.4rem 0.55rem;
		border: 1px solid rgba(50, 112, 76, 0.18);
		border-radius: 8px;
		background: #fff;
		color: #17202a;
		text-align: left;
	}

	.user-protocol-chip small {
		white-space: nowrap;
	}

	.user-protocol-more {
		padding: 0 0.2rem;
	}

	.custom-rules-card {
		justify-self: stretch;
		display: grid;
		gap: 0.14rem;
		min-height: 4.4rem;
		padding: 0.62rem 0.7rem;
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 8px;
		background: #fbfaf8;
		color: #17202a;
		text-align: left;
	}

	.custom-rules-card {
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.75rem;
	}

	.custom-rules-card.active {
		border-color: #bd4a17;
		background: rgba(189, 74, 23, 0.08);
	}

	.custom-rules-card span,
	.custom-rules-card small,
	.protocol-row-main small,
	.protocol-row-meta small,
	.protocol-detail p,
	.detail-item code,
	.detail-item em {
		color: #5f6c79;
		font-size: 0.78rem;
		font-weight: 700;
	}

	.custom-rules-card small {
		font-weight: 650;
	}

	.custom-rules-card > div:first-child {
		display: grid;
		gap: 0.14rem;
		min-width: 0;
	}

	.custom-rule-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.45rem;
		justify-content: end;
	}

	.protocol-list {
		display: grid;
		gap: 0.46rem;
	}

	.watchlist-tools {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 0.6rem;
		align-items: end;
	}

	.watchlist-search {
		min-width: 0;
	}

	.source-filter-row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.35rem;
		justify-content: end;
	}

	.source-filter-button {
		display: inline-flex;
		align-items: center;
		gap: 0.34rem;
		min-height: 2.65rem;
		white-space: nowrap;
	}

	.source-filter-button.active {
		border-color: rgba(189, 74, 23, 0.45);
		background: rgba(189, 74, 23, 0.06);
		color: #8f3d19;
	}

	.source-filter-button span {
		color: #5f6c79;
		font-size: 0.72rem;
		font-weight: 800;
	}

	.protocol-entry {
		display: grid;
		overflow: hidden;
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 8px;
		background: #fff;
	}

	.protocol-entry.open {
		border-color: rgba(189, 74, 23, 0.42);
	}

	.protocol-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: stretch;
	}

	.protocol-row-main {
		justify-self: stretch;
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto auto;
		align-items: center;
		gap: 0.75rem;
		min-height: 3.25rem;
		padding: 0.64rem 0.72rem;
		border: 0;
		border-radius: 0;
		background: #fff;
		color: #17202a;
		text-align: left;
	}

	.protocol-entry.open .protocol-row-main {
		background: rgba(189, 74, 23, 0.04);
	}

	.protocol-row-main > span,
	.protocol-row-meta {
		display: grid;
		gap: 0.14rem;
		min-width: 0;
	}

	.protocol-row-meta {
		justify-items: end;
		text-align: right;
	}

	.configure-label {
		padding-left: 0.55rem;
		border-left: 1px solid rgba(23, 32, 42, 0.1);
		color: #8f3d19;
		font-size: 0.78rem;
		font-weight: 800;
		white-space: nowrap;
	}

	.row-remove-button {
		align-self: center;
		margin-right: 0.5rem;
	}

	.protocol-detail {
		display: grid;
		gap: 0.8rem;
		padding: 0.76rem;
		border-top: 1px solid rgba(23, 32, 42, 0.08);
		background: #fbfaf8;
	}

	.protocol-detail-note {
		margin: 0;
		color: #5f6c79;
		font-size: 0.82rem;
		font-weight: 650;
		line-height: 1.45;
	}

	.protocol-detail-controls {
		display: grid;
		grid-template-columns: minmax(10rem, 14rem) minmax(0, 1fr);
		gap: 0.75rem;
		align-items: center;
	}

	.severity-control {
		min-width: 0;
	}

	.detail-section {
		display: grid;
		gap: 0.5rem;
		min-width: 0;
	}

	.detail-section-head {
		display: flex;
		justify-content: space-between;
		gap: 0.75rem;
		align-items: baseline;
		color: #17202a;
	}

	.detail-section-head span {
		color: #5f6c79;
		font-size: 0.78rem;
		font-weight: 750;
		text-align: right;
	}

	.detail-items {
		display: grid;
		gap: 0.42rem;
	}

	.detail-item {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.6rem;
		min-height: 2.8rem;
		padding: 0.52rem 0.62rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		background: #fff;
	}

	.detail-item.static {
		grid-template-columns: minmax(0, 1fr) auto;
	}

	.detail-item input {
		accent-color: #bd4a17;
	}

	.detail-item span {
		display: grid;
		gap: 0.08rem;
		min-width: 0;
	}

	.detail-item code {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.metric-rule-list {
		display: grid;
		gap: 0.5rem;
	}

	.metric-rule-item {
		display: grid;
		gap: 0.58rem;
		padding: 0.62rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		background: #fff;
	}

	.metric-rule-head,
	.metric-rule-footer {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 0.75rem;
		align-items: center;
	}

	.metric-rule-head span {
		display: grid;
		gap: 0.08rem;
		min-width: 0;
	}

	.metric-rule-head code,
	.metric-rule-empty,
	.metric-rule-footer span {
		color: #5f6c79;
		font-size: 0.78rem;
		font-weight: 700;
		line-height: 1.4;
	}

	.metric-rule-head code {
		overflow-wrap: anywhere;
	}

	.metric-rule-head em {
		color: #8f3d19;
		font-size: 0.78rem;
		font-style: normal;
		font-weight: 800;
	}

	.metric-rule-controls {
		display: grid;
		grid-template-columns: minmax(0, 1.25fr) minmax(0, 1fr);
		gap: 0.55rem;
	}

	.metric-bound-row {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 0.45rem;
	}

	.metric-rule-empty {
		margin: 0;
		padding: 0.55rem 0.6rem;
		border: 1px dashed rgba(23, 32, 42, 0.16);
		border-radius: 8px;
		background: #fbfaf8;
	}

	.billing-row input,
	.delivery-row input,
	.choice-pill input {
		margin-top: 0.2rem;
		accent-color: #bd4a17;
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

	.wallet-connect-card,
	.target-add-row,
	.target-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.7rem;
		padding: 0.72rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		background: #fbfaf8;
	}

	.wallet-connect-card div,
	.target-row div,
	.review-group {
		display: grid;
		gap: 0.16rem;
		min-width: 0;
	}

	.wallet-connect-card span,
	.target-row span,
	.target-list p {
		color: #5f6c79;
		font-size: 0.82rem;
		font-weight: 700;
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

	.wallet-connect-actions,
	.wallet-tools {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		align-items: center;
		justify-content: end;
	}

	.target-add-row {
		grid-template-columns: minmax(0, 1fr) auto;
		background: #fff;
	}

	.target-list {
		display: grid;
		gap: 0.52rem;
	}

	.target-row code {
		font-size: 0.8rem;
		color: #17202a;
	}

	.target-list p {
		margin: 0;
		padding: 0.72rem;
		border: 1px dashed rgba(23, 32, 42, 0.14);
		border-radius: 8px;
		background: #fbfaf8;
	}

	h2,
	p {
		margin: 0;
	}

	h2 {
		font-size: 1.05rem;
	}

	.section-head p,
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

	input[type='search'],
	input[type='text'],
	input[type='number'],
	select {
		width: 100%;
		min-height: 2.65rem;
		padding: 0.68rem;
		border-radius: 8px;
		border: 1px solid rgba(23, 32, 42, 0.12);
		background: #fff;
		color: #17202a;
		font: inherit;
		font-size: 0.9rem;
	}

	input[type='search']:focus,
	input[type='text']:focus,
	input[type='number']:focus,
	select:focus {
		outline: 2px solid rgba(189, 74, 23, 0.2);
		border-color: rgba(189, 74, 23, 0.55);
	}

	.modal-backdrop {
		position: fixed;
		inset: 0;
		z-index: 80;
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

	.confirm-modal {
		width: min(28rem, 100%);
		display: grid;
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

	.confirm-summary {
		display: grid;
		gap: 0.45rem;
	}

	.confirm-summary div {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.75rem;
		padding: 0.58rem 0.65rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		background: #fbfaf8;
	}

	.confirm-summary span {
		color: #5f6c79;
		font-size: 0.82rem;
		font-weight: 700;
	}

	.confirm-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		justify-content: end;
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
		grid-auto-rows: max-content;
		align-content: start;
		gap: 0.5rem;
		padding-right: 0.15rem;
	}

	.program-picker-row {
		grid-template-columns: minmax(0, 1fr) auto;
	}

	.program-picker-row input {
		accent-color: #bd4a17;
	}

	.program-picker-main {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		align-items: center;
		gap: 0.55rem;
		min-width: 0;
		cursor: pointer;
	}

	.program-add-button {
		justify-self: end;
	}

	.billing-row,
	.delivery-row {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 0.55rem;
	}

	.choice-pill,
	.billing-row label {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		align-items: start;
		column-gap: 0.45rem;
		row-gap: 0.08rem;
		padding: 0.62rem 0.7rem;
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 8px;
		background: #fff;
		cursor: pointer;
	}

	.choice-pill:has(input:checked),
	.billing-row label:has(input:checked) {
		border-color: rgba(189, 74, 23, 0.4);
		background: rgba(189, 74, 23, 0.06);
	}

	.billing-row label span,
	.billing-row label small,
	.choice-pill span,
	.choice-pill small {
		grid-column: 2;
	}

	.billing-row label small,
	.choice-pill small {
		margin: 0;
	}

	.review-group {
		display: grid;
		gap: 0.5rem;
	}

	.review-group-head {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 0.75rem;
	}

	.review-group-head span {
		color: #5f6c79;
		font-size: 0.78rem;
		font-weight: 700;
		text-align: right;
	}

	.activation-status {
		display: grid;
		gap: 0.16rem;
		padding: 0.72rem 0.78rem;
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 8px;
		background: #fbfaf8;
	}

	.activation-status.checkout {
		border-color: rgba(189, 74, 23, 0.28);
		background: rgba(189, 74, 23, 0.06);
	}

	.activation-status.paid {
		border-color: rgba(50, 112, 76, 0.2);
		background: rgba(50, 112, 76, 0.06);
	}

	.activation-status span,
	.review-note {
		color: #5f6c79;
		font-size: 0.82rem;
		font-weight: 700;
		line-height: 1.35;
	}

	.save-summary {
		display: grid;
		gap: 0.5rem;
	}

	.save-summary div {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: baseline;
		gap: 0.75rem;
		padding: 0.62rem 0;
		border-bottom: 1px solid rgba(23, 32, 42, 0.08);
	}

	.save-summary div:last-child {
		border-bottom: 0;
	}

	.save-summary span {
		color: #5f6c79;
		font-size: 0.82rem;
		font-weight: 700;
	}

	.save-summary strong {
		text-align: right;
	}

	.payment-summary,
	.payment-link-list,
	.payment-action-row {
		display: grid;
		gap: 0.5rem;
		min-width: 0;
	}

	.payment-summary div,
	.payment-link-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: baseline;
		gap: 0.75rem;
		padding: 0.62rem 0;
		border-bottom: 1px solid rgba(23, 32, 42, 0.08);
		min-width: 0;
	}

	.payment-summary div:last-child {
		border-bottom: 0;
	}

	.payment-summary span,
	.payment-link-row span,
	.payment-action-row span {
		color: #5f6c79;
		font-size: 0.82rem;
		font-weight: 700;
		line-height: 1.35;
	}

	.payment-summary strong,
	.payment-link-row strong {
		text-align: right;
		overflow-wrap: anywhere;
	}

	.payment-link-list {
		padding: 0.12rem 0;
	}

	.payment-link-row {
		align-items: center;
		padding: 0.58rem 0;
	}

	.payment-link-row div {
		display: grid;
		gap: 0.12rem;
		min-width: 0;
	}

	.payment-link-row div strong,
	.payment-link-row div span {
		text-align: left;
	}

	.payment-link-row a {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 2rem;
		padding: 0 0.68rem;
		border: 1px solid rgba(189, 74, 23, 0.3);
		border-radius: 8px;
		background: rgba(189, 74, 23, 0.06);
		color: #8f3a16;
		font-size: 0.8rem;
		font-weight: 850;
		text-decoration: none;
	}

	.payment-create-button {
		justify-self: stretch;
	}

	.mobile-payment-strip div {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: baseline;
		gap: 0.75rem;
		min-width: 0;
	}

	.mobile-payment-strip p {
		margin: 0;
		color: #5f6c79;
		font-size: 0.82rem;
		font-weight: 700;
		line-height: 1.35;
	}

	.mobile-payment-strip span {
		color: #5f6c79;
		font-size: 0.78rem;
		font-weight: 750;
		text-align: right;
	}

	.mobile-payment-strip a {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 2.35rem;
		padding: 0 0.88rem;
		border: 1px solid #bd4a17;
		border-radius: 8px;
		background: #bd4a17;
		color: #fff;
		font-size: 0.84rem;
		font-weight: 850;
		text-decoration: none;
	}

	.review-note {
		margin: 0;
		padding: 0.62rem 0.7rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		background: #fff;
	}

	.review-note.success {
		border-color: rgba(50, 112, 76, 0.18);
		background: rgba(50, 112, 76, 0.06);
		color: #32704c;
	}

	.checkout-panel > button {
		justify-self: stretch;
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

	.danger-button {
		border-color: rgba(156, 57, 41, 0.35);
		background: rgba(156, 57, 41, 0.08);
		color: #9c3929;
	}

	button.danger-button:not(.secondary-button) {
		background: #9c3929;
		color: #fff;
	}

	.list-toggle-button {
		justify-self: stretch;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 2.35rem;
		width: 100%;
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
			padding: 0.75rem 0 calc(6rem + env(safe-area-inset-bottom));
			gap: 0.75rem;
		}

		.panel {
			padding: 0.86rem;
			border-radius: 8px;
		}

		.setup-intro,
		.field-panel,
		.checkout-panel,
		.config-main,
		.config-side {
			gap: 0.75rem;
		}

		.config-layout,
		.program-filters {
			grid-template-columns: 1fr;
		}

		.mobile-payment-strip {
			display: grid;
			gap: 0.55rem;
		}

		.setup-intro {
			grid-template-columns: 1fr;
		}

		.setup-status {
			justify-items: start;
			text-align: left;
		}

		button {
			min-height: 3rem;
			justify-self: stretch;
		}

		.secondary-button {
			min-height: 2.75rem;
		}

		input[type='search'],
		input[type='text'],
		input[type='number'],
		select {
			min-width: 0;
			font-size: 16px;
		}

		.protocol-detail-controls {
			grid-template-columns: 1fr;
			align-items: stretch;
		}

		.billing-row label,
		.choice-pill {
			min-height: 3rem;
		}

		.checkout-panel > button[type='submit'] {
			position: fixed;
			left: 0.75rem;
			right: 0.75rem;
			bottom: calc(0.75rem + env(safe-area-inset-bottom));
			z-index: 50;
			width: auto;
			min-height: 3.15rem;
			box-shadow: 0 6px 18px rgba(23, 32, 42, 0.18);
		}

		:global(.site-footer) {
			padding-bottom: calc(5.75rem + env(safe-area-inset-bottom));
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

		.confirm-modal {
			align-self: center;
			justify-self: center;
			width: min(28rem, calc(100vw - 1rem));
		}

		.program-picker-list {
			overscroll-behavior: contain;
			-webkit-overflow-scrolling: touch;
			padding-right: 0;
		}

		.program-add-button,
		.modal-head button {
			min-height: 2.35rem;
		}
	}

	@media (max-width: 560px) {
		.wallet-connect-card,
		.target-add-row,
		.target-row,
		.billing-row,
		.delivery-row,
		.discovery-action,
		.discovery-options,
		.program-picker-row,
		.watchlist-tools {
			grid-template-columns: 1fr;
		}

		.wallet-connect-actions,
		.wallet-tools,
		.protocol-head-actions,
		.protocol-head-actions .secondary-button,
		.custom-rule-actions,
		.custom-rule-actions .secondary-button,
		.confirm-actions,
		.confirm-actions button,
		.target-row button,
		.protocol-head button,
		.discovery-action button,
		.user-protocols-head button,
		.user-protocol-chip,
		.program-add-button,
		.source-filter-row,
		.source-filter-button {
			width: 100%;
			justify-content: stretch;
		}

		.source-filter-row {
			display: grid;
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}

		.confirm-actions {
			display: grid;
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 680px) {
		.discovery-head,
		.discovery-options,
		.custom-rules-card,
		.protocol-head,
		.user-protocols-head,
		.protocol-row,
		.protocol-row-main {
			grid-template-columns: 1fr;
		}

		.custom-rule-actions {
			justify-content: stretch;
		}

		.protocol-head-actions {
			justify-content: stretch;
		}

		.protocol-row-meta {
			justify-items: start;
			text-align: left;
		}

		.metric-rule-controls,
		.metric-rule-head,
		.metric-rule-footer {
			grid-template-columns: 1fr;
		}

		.configure-label {
			padding-left: 0;
			border-left: 0;
		}

		.row-remove-button {
			width: 100%;
		}
	}
</style>
