<script lang="ts">
	import { browser } from '$app/environment';
	import { resolve } from '$app/paths';
	import { onMount } from 'svelte';
	import ProtocolActivityCard from '$lib/components/ProtocolActivityCard.svelte';
	import RollingNumber from '$lib/components/RollingNumber.svelte';
	import { compactLongTokens } from '$lib/event-preview';
	import { getLiveStatusContext } from '$lib/live-status-context.svelte';
	import type { ChainEvent, DefiProtocolDescriptor, EventsResponse, EventsSummary } from '$lib/types';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	type AlertSeverity = 'critical' | 'high' | 'standard';

	type ProtocolAlertRow = {
		slug: string;
		name: string;
		recentAlertCount: number;
		criticalCount: number;
		highCount: number;
		standardCount: number;
		latestSlot: number;
		latestTimestamp: number | null;
		latestSummary: string;
		latestEventId: string;
	};

	type ProtocolRow = {
		protocol: DefiProtocolDescriptor;
		alerts: ProtocolAlertRow | null;
	};

	type SummaryMetric = {
		label: string;
		value: number | null | undefined;
		detail: string;
		detailParts?: string[];
		detailTone?: 'good' | 'caution' | 'neutral';
		href: '/events' | '/events?scope=critical' | '/protocols';
		ariaLabel: string;
	};

	type WatchedSourceStats = {
		protocols: number;
		programs: number;
		multisigs: number;
		accounts: number;
	};

	type ProcessedTxSample = {
		count: number;
		sampledAtMs: number;
	};

	const DEFAULT_BOT_USERNAME = 'fernowatch_bot';
	const HOME_LIVE_REFRESH_MS = 8_000;
	const HOME_LIVE_WINDOW_SECONDS = 7 * 86_400;
	const HOME_PROTOCOL_LIMIT = 10;
	const liveStatusContext = getLiveStatusContext();

	let refreshedProtocolAlertRows = $state<ProtocolAlertRow[] | null>(null);
	let refreshedWindowAlertCount = $state<number | null>(null);
	let refreshedWindowCriticalCount = $state<number | null>(null);
	let refreshedWindowMultisigCount = $state<number | null>(null);
	let refreshedHeroAlert = $state<PageData['homeHeroAlert'] | undefined>(undefined);
	let isHomeRefreshing = $state(false);
	let homeRefreshError = $state<string | null>(null);
	let lastHomeRefreshAt = $state<number | null>(Date.now());
	let liveClock = $state(Date.now());
	let processedTxBaseCount = $state<number | null>(
		liveStatusContext.summary()?.processed_transaction_count ?? null
	);
	let processedTxBaseSampledAtMs = $state(liveStatusContext.liveStatus?.sampled_at_ms ?? Date.now());
	let processedTxRatePerMs = $state(0);
	let previousProcessedTxSample: ProcessedTxSample | null = null;
	let observedProcessedTxRatePerMs = 0;
	const liveProtocolAlertRows = $derived(refreshedProtocolAlertRows ?? data.homeProtocolAlertRows);
	const liveWindowAlertCount = $derived(refreshedWindowAlertCount ?? data.homeActivityWindow.current);
	const liveWindowCriticalCount = $derived(
		refreshedWindowCriticalCount ?? data.homeCriticalActivityWindow.current
	);
	const liveWindowMultisigCount = $derived(
		refreshedWindowMultisigCount ?? data.homeRelatedMultisigCount
	);
	const liveHeroAlert = $derived(
		refreshedHeroAlert === undefined ? data.homeHeroAlert : refreshedHeroAlert
	);
	const liveProcessedTransactionCount = $derived.by(() => {
		const baseCount =
			processedTxBaseCount ?? liveStatusContext.summary()?.processed_transaction_count ?? 0;
		const elapsedMs = Math.min(
			Math.max(0, liveClock - processedTxBaseSampledAtMs),
			HOME_LIVE_REFRESH_MS * 2
		);

		return Math.floor(baseCount + processedTxRatePerMs * elapsedMs);
	});

	const alertRowsBySlug = $derived.by(() => {
		const rows: Record<string, ProtocolAlertRow> = {};
		for (const row of liveProtocolAlertRows) {
			rows[row.slug] = row;
		}
		return rows;
	});

	const protocolRows = $derived.by<ProtocolRow[]>(() =>
		data.defiProtocols
			.map((protocol) => ({
				protocol,
				alerts: alertRowsBySlug[protocol.slug] ?? null
			}))
	);
	const topProtocolRows = $derived.by(() =>
		[...protocolRows]
			.sort(protocolActivitySort)
			.slice(0, HOME_PROTOCOL_LIMIT)
	);
	const watchedSourceStats = $derived.by<WatchedSourceStats>(() => {
		const programs = new Set<string>();
		const accounts = new Set<string>();
		const multisigs = new Set<string>();

		for (const protocol of data.defiProtocols) {
			if (protocol.program_id) programs.add(protocol.program_id);

			for (const program of protocol.programs) {
				if (program.program_id) programs.add(program.program_id);
				if (program.idl_metadata_address) accounts.add(program.idl_metadata_address);
				if (program.anchor_legacy_idl_address) accounts.add(program.anchor_legacy_idl_address);
			}

			for (const domain of protocol.sol_domains) {
				if (domain) accounts.add(domain);
			}

			for (const link of protocol.related_multisigs ?? []) {
				if (link.multisig) multisigs.add(link.multisig);
			}
		}

		return {
			protocols: data.defiProtocols.length,
			programs: programs.size,
			multisigs: Math.max(multisigs.size, liveWindowMultisigCount),
			accounts: accounts.size
		};
	});

	const summaryMetrics = $derived.by<SummaryMetric[]>(() => {
		const liveSummary = liveStatusContext.summary();
		const liveStatus = liveStatusContext.liveStatus;
		const slotTip = estimatedSlotTip(
			liveSummary,
			liveStatus?.sampled_at_ms,
			liveStatus?.estimated_slot_time_ms,
			liveClock
		);
		const startSlot = liveSummary?.backfill_start_slot ?? liveSummary?.first_indexed_slot ?? null;

		return [
			{
				label: `${data.homeActivityWindow.days}d alerts monitored`,
				value: liveWindowAlertCount,
				detail: windowChangeLabel(data.homeActivityWindow, liveWindowAlertCount),
				detailTone: windowChangeTone(data.homeActivityWindow, liveWindowAlertCount),
				href: '/events',
				ariaLabel: 'Open live feed'
			},
			{
				label: `${data.homeCriticalActivityWindow.days}d critical alerts`,
				value: liveWindowCriticalCount,
				detail: windowChangeLabel(
					data.homeCriticalActivityWindow,
					liveWindowCriticalCount
				),
				detailTone: windowChangeTone(
					data.homeCriticalActivityWindow,
					liveWindowCriticalCount
				),
				href: '/events?scope=critical',
				ariaLabel: 'Open critical alerts feed'
			},
				{
					label: 'Live tx handled',
					value: liveProcessedTransactionCount,
					detail:
						slotTip == null || startSlot == null
							? 'Waiting for slots'
							: `Slot tip ${formatCount(slotTip)} / Start ${formatCount(startSlot)}`,
					detailParts:
						slotTip == null || startSlot == null
							? undefined
							: [`Slot tip ${formatCount(slotTip)}`, `Start ${formatCount(startSlot)}`],
					detailTone: 'neutral',
					href: '/events',
					ariaLabel: 'Open live feed'
				},
			{
					label: 'Protocols watched',
					value: watchedSourceStats.protocols,
					detail: watchedSourceDetail(watchedSourceStats),
					detailParts: watchedSourceDetailParts(watchedSourceStats),
					detailTone: 'neutral',
					href: '/protocols',
					ariaLabel: 'Open protocols'
				}
		];
	});

	$effect(() => {
		const liveStatus = liveStatusContext.liveStatus;
		const nextCount = liveStatus?.summary.processed_transaction_count;
		if (!liveStatus || nextCount == null) return;

		const sampledAtMs = liveStatus.sampled_at_ms || Date.now();
		if (
			previousProcessedTxSample &&
			sampledAtMs > previousProcessedTxSample.sampledAtMs &&
			nextCount >= previousProcessedTxSample.count
		) {
			const nextRate =
				(nextCount - previousProcessedTxSample.count) /
				(sampledAtMs - previousProcessedTxSample.sampledAtMs);
			if (Number.isFinite(nextRate)) {
				observedProcessedTxRatePerMs =
					nextRate > 0
						? observedProcessedTxRatePerMs > 0
							? observedProcessedTxRatePerMs * 0.35 + nextRate * 0.65
							: nextRate
						: 0;
				processedTxRatePerMs = observedProcessedTxRatePerMs;
			}
		}

		processedTxBaseCount = nextCount;
		processedTxBaseSampledAtMs = sampledAtMs;
		previousProcessedTxSample = { count: nextCount, sampledAtMs };
	});

	$effect(() => {
		data.homeProtocolAlertRows;
		data.homeActivityWindow.current;
		data.homeCriticalActivityWindow.current;
		data.homeRelatedMultisigCount;
		refreshedProtocolAlertRows = null;
		refreshedWindowAlertCount = null;
		refreshedWindowCriticalCount = null;
		refreshedWindowMultisigCount = null;
		refreshedHeroAlert = undefined;
		homeRefreshError = null;
	});

	onMount(() => {
		if (!browser) return;

		void refreshHomeLiveData();

		const refreshInterval = window.setInterval(() => {
			void refreshHomeLiveData();
		}, HOME_LIVE_REFRESH_MS);
		const clockInterval = window.setInterval(() => {
			liveClock = Date.now();
		}, 1_000);

		function refreshWhenVisible() {
			if (document.visibilityState === 'visible') {
				liveClock = Date.now();
				void refreshHomeLiveData();
			}
		}

		document.addEventListener('visibilitychange', refreshWhenVisible);

		return () => {
			window.clearInterval(refreshInterval);
			window.clearInterval(clockInterval);
			document.removeEventListener('visibilitychange', refreshWhenVisible);
		};
	});

	function botUsername() {
		const username = liveStatusContext.liveStatus?.telegram_bot?.username?.trim();
		return username || DEFAULT_BOT_USERNAME;
	}

	function botHandle() {
		return `@${botUsername()}`;
	}

	function addBotUrl() {
		return `https://t.me/${botUsername()}?startgroup=setup`;
	}

	function openAddBot() {
		window.open(addBotUrl(), '_blank', 'noopener,noreferrer');
	}

	function formatCount(value: number | null | undefined) {
		return new Intl.NumberFormat('en-US').format(value ?? 0);
	}

	function estimatedSlotTip(
		summary: EventsSummary | null,
		sampledAtMs: number | undefined,
		estimatedSlotTimeMs: number | undefined,
		nowMs: number
	) {
		const baseSlot = summary?.last_processed_slot;
		if (baseSlot == null) return null;
		if (!sampledAtMs) return baseSlot;

		const slotTimeMs = Math.max(1, estimatedSlotTimeMs ?? 1000);
		const elapsedMs = Math.max(0, nowMs - sampledAtMs);
		return baseSlot + Math.floor(elapsedMs / slotTimeMs);
	}

	function windowChangeLabel(window: {
		days: number;
		previous: number;
	}, current = 0) {
		const { days, previous } = window;
		const percentChange = livePercentChange(current, previous);
		if (percentChange == null) return `New vs previous ${days}d`;
		if (percentChange === 0) return `Same as previous ${days}d`;
		const direction = percentChange > 0 ? 'more' : 'fewer';
		return `${Math.abs(percentChange)}% ${direction} than previous ${days}d`;
	}

	function windowChangeTone(window: {
		previous: number;
	}, current = 0): SummaryMetric['detailTone'] {
		const percentChange = livePercentChange(current, window.previous);
		if (percentChange == null || percentChange === 0) return 'neutral';
		return percentChange < 0 ? 'good' : 'caution';
	}

	function livePercentChange(current: number, previous: number) {
		if (previous === 0) return current === 0 ? 0 : null;
		return Math.round(((current - previous) / previous) * 100);
	}

	function watchedSourceDetail(stats: WatchedSourceStats) {
		return watchedSourceDetailParts(stats).join(' / ');
	}

	function watchedSourceDetailParts(stats: WatchedSourceStats) {
		return [
			pluralLabel(stats.programs, 'program'),
			pluralLabel(stats.multisigs, 'multisig'),
			pluralLabel(stats.accounts, 'account')
		];
	}

	function protocolActivitySort(left: ProtocolRow, right: ProtocolRow) {
		const nameSort = left.protocol.name.localeCompare(right.protocol.name);
		return (
			(right.alerts?.recentAlertCount ?? 0) - (left.alerts?.recentAlertCount ?? 0) ||
			totalAlertCount(right.protocol.slug) - totalAlertCount(left.protocol.slug) ||
			(right.alerts?.criticalCount ?? 0) - (left.alerts?.criticalCount ?? 0) ||
			(right.alerts?.latestTimestamp ?? 0) - (left.alerts?.latestTimestamp ?? 0) ||
			nameSort
		);
	}

	function pluralLabel(count: number, singular: string, plural = `${singular}s`) {
		return `${formatCount(count)} ${count === 1 ? singular : plural}`;
	}

	function trackedSourceParts(protocol: DefiProtocolDescriptor) {
		const multisigCount = uniqueRelatedMultisigCount(protocol);
		return [
			protocol.sol_domains.length > 0 ? pluralLabel(protocol.sol_domains.length, 'wallet') : null,
			protocol.programs.length > 0 ? pluralLabel(protocol.programs.length, 'program') : null,
			multisigCount > 0 ? pluralLabel(multisigCount, 'multisig') : null,
			protocol.amount_triggers.length > 0 ? pluralLabel(protocol.amount_triggers.length, 'rule') : null
		].filter((part): part is string => Boolean(part));
	}

	function uniqueRelatedMultisigCount(protocol: DefiProtocolDescriptor) {
		return new Set((protocol.related_multisigs ?? []).map((link) => link.multisig).filter(Boolean)).size;
	}

	function trackedSourceLabel(protocol: DefiProtocolDescriptor) {
		const parts = trackedSourceParts(protocol);
		return parts.length > 0 ? parts.join(', ') : 'No tracked items';
	}

	function protocolCardMetrics(row: ProtocolRow) {
		return [
			{ label: 'Total alerts', value: totalAlertCount(row.protocol.slug) },
			{ label: `${data.homeActivityWindow.days}d Alerts`, value: row.alerts?.recentAlertCount },
			{ label: `${data.homeActivityWindow.days}d Critical`, value: row.alerts?.criticalCount }
		];
	}

	function totalAlertCount(slug: string) {
		return data.homeProtocolTotals[slug] ?? 0;
	}

	function homeLiveRefreshParams() {
		const nowTs = Math.floor(Date.now() / 1000);
		return new URLSearchParams({
			'page[size]': '250',
			'page[offset]': '0',
			scope: 'defi',
			include_related_multisigs: 'true',
			from_ts: String(nowTs - HOME_LIVE_WINDOW_SECONDS),
			before_ts: String(nowTs)
		});
	}

	function homeLiveUrl() {
		return `${resolve('/events/live')}?${homeLiveRefreshParams().toString()}`;
	}

	async function refreshHomeLiveData() {
		if (!browser || isHomeRefreshing) return;
		if (document.visibilityState !== 'visible') return;

		isHomeRefreshing = true;
		homeRefreshError = null;

		try {
			const response = await fetch(homeLiveUrl());
			if (!response.ok) throw new Error(`Backend returned ${response.status}`);

			const payload = (await response.json()) as EventsResponse;
			refreshedProtocolAlertRows = protocolAlertRowsFromEvents(payload.items);
			refreshedWindowAlertCount = responseTotalCount(payload);
			refreshedWindowCriticalCount = responseCriticalCount(payload);
			refreshedWindowMultisigCount = distinctMultisigCount(payload.items);
			refreshedHeroAlert = heroAlertFromEvents(payload.items);
			lastHomeRefreshAt = Date.now();
		} catch (error) {
			homeRefreshError = error instanceof Error ? error.message : 'Could not refresh home activity';
		} finally {
			isHomeRefreshing = false;
		}
	}

	function responseTotalCount(payload: EventsResponse) {
		return payload.total_count ?? payload.summary?.total_count ?? payload.items.length;
	}

	function responseCriticalCount(payload: EventsResponse) {
		return payload.scope_counts?.critical ?? payload.items.filter((event) => eventSeverity(event) === 'critical').length;
	}

	function eventMultisigAddress(event: ChainEvent) {
		const multisig = payloadString(event, 'multisig');
		if (multisig) return multisig;
		return event.target_type === 'multisig' ? event.target_address : null;
	}

	function distinctMultisigCount(events: ChainEvent[]) {
		return new Set(events.map(eventMultisigAddress).filter((value): value is string => Boolean(value))).size;
	}

	function payloadString(event: ChainEvent, key: string) {
		const value = event.payload[key];
		return typeof value === 'string' && value.length > 0 ? value : null;
	}

	function eventTimestamp(event: ChainEvent) {
		return event.block_time ?? event.indexed_at;
	}

	function eventSeverity(event: ChainEvent): AlertSeverity {
		const payloadSeverity = payloadString(event, 'severity');
		if (
			payloadSeverity === 'critical' ||
			event.tags.includes('severity:critical') ||
			event.tags.includes('signal:unknown_instruction')
		) {
			return 'critical';
		}

		if (
			payloadSeverity === 'high' ||
			event.tags.includes('severity:high') ||
			event.tags.includes('impact:high')
		) {
			return 'high';
		}

		return 'standard';
	}

	function linkedProtocolSlugs(event: ChainEvent) {
		const linked = event.payload.linked_protocols;
		if (!Array.isArray(linked)) return [];

		return linked
			.map((item) => {
				if (!item || typeof item !== 'object') return null;
				const protocol = (item as Record<string, unknown>).protocol;
				return typeof protocol === 'string' ? protocol : null;
			})
			.filter((value): value is string => Boolean(value));
	}

	function eventProtocolSlugs(event: ChainEvent, knownProtocolSlugs: Set<string>) {
		const slugs = new Set<string>();
		for (const tag of event.tags) {
			if (!tag.startsWith('protocol:')) continue;
			const slug = tag.slice('protocol:'.length);
			if (knownProtocolSlugs.has(slug)) slugs.add(slug);
		}

		for (const slug of [
			payloadString(event, 'protocol'),
			payloadString(event, 'decoded_protocol'),
			payloadString(event, 'protocol_slug'),
			...linkedProtocolSlugs(event)
		]) {
			if (slug && knownProtocolSlugs.has(slug)) slugs.add(slug);
		}

		return [...slugs];
	}

	function protocolAlertRowsFromEvents(events: ChainEvent[]) {
		const protocolNames = new Map(data.defiProtocols.map((protocol) => [protocol.slug, protocol.name]));
		const knownProtocolSlugs = new Set(protocolNames.keys());
		const rows = new Map<string, ProtocolAlertRow>();

		for (const event of events) {
			const severity = eventSeverity(event);
			for (const slug of eventProtocolSlugs(event, knownProtocolSlugs)) {
				const row =
					rows.get(slug) ??
					({
						slug,
						name: protocolNames.get(slug) ?? slug.replaceAll('_', ' '),
						recentAlertCount: 0,
						criticalCount: 0,
						highCount: 0,
						standardCount: 0,
						latestSlot: 0,
						latestTimestamp: null,
						latestSummary: '',
						latestEventId: ''
					} satisfies ProtocolAlertRow);

				row.recentAlertCount += 1;
				if (severity === 'critical') row.criticalCount += 1;
				if (severity === 'high') row.highCount += 1;
				if (severity === 'standard') row.standardCount += 1;
				if (event.slot > row.latestSlot) {
					row.latestSlot = event.slot;
					row.latestTimestamp = eventTimestamp(event);
					row.latestSummary = compactLongTokens(event.summary);
					row.latestEventId = event.id;
				}

				rows.set(slug, row);
			}
		}

		return [...rows.values()].sort(
			(left, right) =>
				right.recentAlertCount - left.recentAlertCount ||
				right.criticalCount - left.criticalCount ||
				right.highCount - left.highCount ||
				right.latestSlot - left.latestSlot ||
				left.name.localeCompare(right.name)
		);
	}

	function alertSeverityRank(severity: AlertSeverity) {
		if (severity === 'critical') return 3;
		if (severity === 'high') return 2;
		return 1;
	}

	function isBetterHeroAlert(
		candidate: NonNullable<PageData['homeHeroAlert']>,
		current: PageData['homeHeroAlert']
	) {
		if (!current) return true;

		const rankDelta = alertSeverityRank(candidate.severity) - alertSeverityRank(current.severity);
		if (rankDelta !== 0) return rankDelta > 0;

		const candidateTimestamp = candidate.timestamp ?? 0;
		const currentTimestamp = current.timestamp ?? 0;
		return candidateTimestamp !== currentTimestamp
			? candidateTimestamp > currentTimestamp
			: candidate.slot > current.slot;
	}

	function heroAlertFromEvents(events: ChainEvent[]): PageData['homeHeroAlert'] {
		const protocolNames = new Map(data.defiProtocols.map((protocol) => [protocol.slug, protocol.name]));
		const knownProtocolSlugs = new Set(protocolNames.keys());
		let selected: PageData['homeHeroAlert'] = null;

		for (const event of events) {
			const slugs = eventProtocolSlugs(event, knownProtocolSlugs);
			if (slugs.length === 0) continue;

			const severity = eventSeverity(event);
			for (const slug of slugs) {
				const candidate: NonNullable<PageData['homeHeroAlert']> = {
					protocolSlug: slug,
					protocolName: protocolNames.get(slug) ?? slug.replaceAll('_', ' '),
					severity,
					summary: compactLongTokens(event.summary),
					eventId: event.id,
					slot: event.slot,
					timestamp: eventTimestamp(event)
				};

				if (isBetterHeroAlert(candidate, selected)) selected = candidate;
			}
		}

		return selected;
	}

	function ageSeconds(timestamp: number | null | undefined) {
		if (!timestamp) return null;
		return Math.max(0, Math.floor((liveClock - timestamp * 1000) / 1000));
	}

	function relativeAge(timestamp: number | null | undefined) {
		const seconds = ageSeconds(timestamp);
		if (seconds == null) return null;
		if (seconds < 45) return 'now';
		if (seconds < 3_600) return `${Math.max(1, Math.floor(seconds / 60))}m ago`;
		if (seconds < 86_400) return `${Math.max(1, Math.floor(seconds / 3_600))}h ago`;
		return `${Math.max(1, Math.floor(seconds / 86_400))}d ago`;
	}

	function alertSeverityLabel(severity: AlertSeverity) {
		if (severity === 'critical') return 'Critical alert';
		if (severity === 'high') return 'High alert';
		return 'Protocol alert';
	}

	function protocolStatusLabel(row: ProtocolRow) {
		const relative = relativeAge(row.alerts?.latestTimestamp);
		if (!relative) return `No alert last ${data.homeActivityWindow.days}d`;
		return `Last alert ${relative}`;
	}

	function protocolStatusClass(row: ProtocolRow) {
		const seconds = ageSeconds(row.alerts?.latestTimestamp);
		if (seconds == null) return 'watching';
		if (seconds < 900) return 'active';
		if (seconds < 86_400) return 'warm';
		return 'quiet';
	}

	function refreshAgeLabel() {
		if (lastHomeRefreshAt == null) return 'Connecting';
		const seconds = Math.max(0, Math.floor((liveClock - lastHomeRefreshAt) / 1000));
		if (seconds <= 1) return 'Updated now';
		return `Updated ${seconds}s ago`;
	}

	function liveStatusUpdateLabel() {
		const sampledAtMs = liveStatusContext.liveStatus?.sampled_at_ms;
		if (!sampledAtMs) return refreshAgeLabel();

		const seconds = Math.max(0, Math.floor((liveClock - sampledAtMs) / 1000));
		if (seconds <= 5) return 'Updated now';
		if (seconds < 60) return `Updated ${seconds}s ago`;
		return `Updated ${Math.floor(seconds / 60)}m ago`;
	}

	function homeLiveStatusLabel() {
		if (liveStatusContext.liveStatusError && !liveStatusContext.liveStatus) return 'Live indexer delayed';
		if (liveStatusContext.summary()?.last_processed_slot != null) return 'Live indexer online';
		if (isHomeRefreshing) return 'Refreshing protocol activity';
		if (homeRefreshError) return 'Protocol activity delayed';
		return 'Indexer connecting';
	}

	function homeLiveDetail() {
		if (liveStatusContext.liveStatusError && !liveStatusContext.liveStatus) return 'Retrying automatically';
		if (homeRefreshError) return 'Protocol activity retrying';
		return liveStatusUpdateLabel();
	}
</script>

<svelte:head>
	<title>FireWatch protocol alerts</title>
</svelte:head>

<div class="home-shell">
	<section class="overview-panel" aria-labelledby="home-title">
		<div class="overview-copy">
			<h1 id="home-title">Live monitoring for protocols you depend on.</h1>
			<p>
				FireWatch tracks wallets, protocols, and the layers behind them, then sends the
				critical alerts you need to act on - so you're never the last to know.
			</p>
			<div class="live-overview" aria-label="Live home status">
				<strong>{homeLiveStatusLabel()}</strong>
				<span>{homeLiveDetail()}</span>
			</div>
			<div class="summary-grid hero-stats" aria-label="Alert summary">
				{#each summaryMetrics as metric (metric.label)}
					<a class="summary-card" href={resolve(metric.href)} aria-label={metric.ariaLabel}>
						<strong><RollingNumber value={metric.value} /></strong>
						<span>{metric.label}</span>
							<small
								class:good-detail={metric.detailTone === 'good'}
								class:caution-detail={metric.detailTone === 'caution'}
							>
								{#if metric.detailParts}
									<span class="summary-detail-parts" aria-label={metric.detail}>
										{#each metric.detailParts as part (part)}
											<span aria-hidden="true">{part}</span>
										{/each}
									</span>
								{:else}
									{metric.detail}
								{/if}
							</small>
						</a>
					{/each}
			</div>
			{#if liveHeroAlert}
				<div class="latest-alert-block">
					<span class="latest-alert-label">Last critical alert</span>
					<a
						class="alert-preview"
						class:critical-preview={liveHeroAlert.severity === 'critical'}
						class:high-preview={liveHeroAlert.severity === 'high'}
						href={resolve('/events?scope=critical')}
						aria-label="Open critical alerts feed"
					>
						<div>
							<div class="alert-preview-topline">
								<strong>{liveHeroAlert.protocolName}</strong>
								<span>{alertSeverityLabel(liveHeroAlert.severity)}</span>
							</div>
							<p>{liveHeroAlert.summary}</p>
						</div>
						<time datetime={new Date((liveHeroAlert.timestamp ?? 0) * 1000).toISOString()}>
							{relativeAge(liveHeroAlert.timestamp) ?? 'Live'}
						</time>
					</a>
				</div>
			{/if}
		</div>

			<aside class="get-started-panel" aria-labelledby="get-started-title">
				<h2 id="get-started-title">Get started</h2>
				<div class="start-offers">
					<article class="start-card" aria-labelledby="telegram-start-title">
						<h3 id="telegram-start-title">Telegram alerts</h3>
						<ol class="start-steps" aria-label="Telegram alert setup">
							<li>
								<div class="step-copy">
									<strong>Add bot</strong>
									<code class="bot-handle">{botHandle()}</code>
								</div>
							</li>
							<li>
								<strong>Run /config or /watch defi</strong>
								<span>Configure your watch or get our curated alerts</span>
							</li>
							<li>
								<strong>Get alerts</strong>
								<span>Wallet, protocol, and dependency changes</span>
							</li>
						</ol>
						<div class="start-card-action">
							<button class="primary-action" type="button" onclick={openAddBot}>
								Open Telegram
							</button>
						</div>
					</article>

					<article class="start-card custom-watch-card" aria-labelledby="custom-watch-title">
						<h3 id="custom-watch-title">Create custom watch</h3>
						<ul class="watch-benefits">
							<li>Private or public protocol page your team can own</li>
							<li>Track programs, wallets, multisigs, accounts, and dependencies</li>
							<li>Triage critical alerts before they page the team</li>
							<li>Escalation routing for PagerDuty, Slack, email, or secure channels</li>
						</ul>
						<div class="start-card-action">
							<a class="secondary-action" href={resolve('/contact?intent=custom-watch')}>Create protocol page</a>
						</div>
					</article>
				</div>
			</aside>
	</section>

	<div class="workspace-grid">
		<section class="panel protocol-panel" aria-labelledby="protocols-title">
			<div class="section-head">
				<div>
					<h2 id="protocols-title">Most active protocols</h2>
					<p>Top 10 by alerts in the last {data.homeActivityWindow.days} days, then total alerts.</p>
				</div>
				<a href={resolve('/protocols')}>All protocols</a>
			</div>

			<div class="protocol-list">
				{#if topProtocolRows.length === 0}
					<p class="empty-state">No active protocol alerts in the last {data.homeActivityWindow.days} days.</p>
				{:else}
					{#each topProtocolRows as row (row.protocol.slug)}
						<ProtocolActivityCard
							protocol={row.protocol}
							metrics={protocolCardMetrics(row)}
							sourceLabel={trackedSourceLabel(row.protocol)}
							sideLabel={protocolStatusLabel(row)}
							statusTone={protocolStatusClass(row)}
						/>
					{/each}
					<a class="protocol-list-more" href={resolve('/protocols')}>See more protocols</a>
				{/if}
			</div>
		</section>

	</div>
</div>

<style>
	.home-shell {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem 0 2.4rem;
		display: grid;
		gap: 0.86rem;
	}

	.overview-panel {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(320px, 420px);
		gap: 1rem;
		align-items: stretch;
		min-width: 0;
		max-width: 100%;
		box-sizing: border-box;
	}

	.overview-copy,
	.panel,
	.summary-card,
	.start-card {
		border: 1px solid var(--fw-border);
		border-radius: 8px;
		background: #fff;
		min-width: 0;
		max-width: 100%;
		box-sizing: border-box;
	}

	.overview-copy {
		display: grid;
		align-content: center;
		gap: 0.86rem;
		min-height: 21rem;
		padding: 1.28rem;
	}

	h1,
	h2,
	p {
		margin: 0;
	}

	h1 {
		max-width: 15ch;
		font-size: 3.75rem;
		line-height: 0.96;
		letter-spacing: 0;
	}

	h2 {
		font-size: 1rem;
		line-height: 1.25;
	}

	p {
		color: var(--fw-muted);
		line-height: 1.5;
	}

	.overview-copy p {
		max-width: 42rem;
		font-size: 1rem;
	}

	.live-overview {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.36rem 0.62rem;
		padding-top: 0.18rem;
		color: var(--fw-muted);
		font-size: 0.82rem;
		font-weight: 680;
	}

	.live-overview strong {
		color: #167044;
		font-size: 0.84rem;
		line-height: 1.2;
	}

	.alert-preview {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 0.82rem;
		align-items: start;
		padding: 0.78rem 0;
		border-top: 1px solid rgba(23, 32, 42, 0.1);
		border-bottom: 1px solid rgba(23, 32, 42, 0.1);
		color: inherit;
		text-decoration: none;
	}

	.alert-preview:hover p {
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.alert-preview-topline {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.42rem;
		margin-bottom: 0.24rem;
	}

	.alert-preview-topline strong {
		font-size: 0.88rem;
		line-height: 1.2;
	}

	.alert-preview-topline span {
		display: inline-flex;
		align-items: center;
		min-height: 1.35rem;
		padding: 0 0.42rem;
		border-radius: 7px;
		border: 1px solid rgba(23, 32, 42, 0.12);
		background: #fbfaf8;
		color: #4f5a64;
		font-size: 0.72rem;
		font-weight: 780;
		line-height: 1;
	}

	.alert-preview.critical-preview .alert-preview-topline span {
		border-color: rgba(156, 57, 41, 0.24);
		background: rgba(191, 74, 57, 0.1);
		color: #9c3929;
	}

	.alert-preview.high-preview .alert-preview-topline span {
		border-color: rgba(173, 78, 34, 0.22);
		background: rgba(173, 78, 34, 0.09);
		color: #9b3f17;
	}

	.alert-preview p {
		max-width: 47rem;
		color: #17202a;
		font-size: 0.92rem;
		font-weight: 760;
		line-height: 1.32;
	}

	.alert-preview time {
		color: var(--fw-muted);
		font-size: 0.78rem;
		font-weight: 760;
		white-space: nowrap;
	}

	.latest-alert-block {
		display: grid;
		gap: 0.24rem;
	}

	.latest-alert-label {
		color: #62707c;
		font-size: 0.72rem;
		font-weight: 760;
		line-height: 1.2;
	}

	.get-started-panel {
		display: grid;
		align-content: start;
		gap: 0.62rem;
		min-width: 0;
	}

	.start-offers {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(17rem, 100%), 1fr));
		gap: 0.64rem;
		align-items: stretch;
	}

	.start-card {
		display: grid;
		grid-template-rows: auto minmax(0, 1fr) auto;
		align-content: stretch;
		gap: 0.64rem;
		padding: 0.86rem;
	}

	.start-card h3 {
		margin: 0;
		color: #17202a;
		font-size: 0.92rem;
		line-height: 1.2;
	}

	.watch-benefits {
		display: grid;
		gap: 0.36rem;
		margin: 0;
		padding-left: 1rem;
		color: #5f6c79;
		font-size: 0.84rem;
		font-weight: 650;
		line-height: 1.36;
	}

	.start-steps {
		counter-reset: start-step;
		display: grid;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.start-steps li {
		counter-increment: start-step;
		position: relative;
		display: grid;
		gap: 0.1rem;
		min-width: 0;
		padding: 0.58rem 0 0.58rem 1.58rem;
		border-top: 1px solid rgba(23, 32, 42, 0.08);
	}

	.start-steps li:first-child {
		padding-top: 0;
		border-top: 0;
	}

	.start-steps li::before {
		content: counter(start-step);
		position: absolute;
		top: 0.58rem;
		left: 0;
		color: var(--fw-accent);
		font-size: 0.78rem;
		font-weight: 820;
		line-height: 1.25;
	}

	.start-steps li:first-child::before {
		top: 0;
	}

	.start-steps strong {
		color: #17202a;
		font-size: 0.82rem;
		line-height: 1.22;
	}

	.start-steps span {
		min-width: 0;
		color: #62707c;
		font-size: 0.76rem;
		font-weight: 650;
		line-height: 1.25;
	}

	.step-copy {
		display: grid;
		gap: 0.1rem;
	}

	.start-card-action {
		display: flex;
		align-items: center;
		padding-top: 0.08rem;
	}

	.bot-handle {
		width: fit-content;
		max-width: 100%;
		padding: 0.04rem 0.24rem;
		border: 1px solid rgba(23, 32, 42, 0.12);
		border-radius: 5px;
		background: #fbfaf8;
		color: #17202a;
		font: inherit;
		font-size: 0.76rem;
		font-weight: 720;
		line-height: 1.25;
		overflow-wrap: anywhere;
	}

	.primary-action,
	.secondary-action {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 2.14rem;
		padding: 0 0.78rem;
		border-radius: 7px;
		font: inherit;
		font-size: 0.84rem;
		font-weight: 780;
		text-decoration: none;
		cursor: pointer;
		width: auto;
		max-width: max-content;
		white-space: nowrap;
	}

	.primary-action {
		border: 1px solid var(--fw-accent);
		background: var(--fw-accent);
		color: #fff;
	}

	.secondary-action {
		border: 0;
		background: transparent;
		color: #4d4036;
	}

	.custom-watch-card .secondary-action {
		border: 1px solid rgba(23, 32, 42, 0.12);
		background: #fff;
	}

	.primary-action:hover {
		background: var(--fw-accent-strong);
		border-color: var(--fw-accent-strong);
	}

	.secondary-action:hover {
		color: #1f2933;
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.summary-grid {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: 0.64rem;
	}

	.hero-stats {
		margin-top: 0.12rem;
	}

	.summary-card {
		display: grid;
		gap: 0.16rem;
		min-width: 0;
		padding: 0.82rem;
		color: inherit;
		text-decoration: none;
	}

	.summary-card:hover {
		border-color: rgba(173, 78, 34, 0.28);
	}

	.summary-card:hover > span {
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.summary-card:focus-visible {
		outline: 3px solid rgba(173, 78, 34, 0.18);
		outline-offset: 2px;
	}

	.summary-card strong {
		font-size: 1.22rem;
		line-height: 1;
		overflow-wrap: anywhere;
	}

	.summary-card > span {
		color: #313b44;
		font-size: 0.82rem;
		font-weight: 760;
	}

	.summary-card > small {
		color: var(--fw-muted);
		font-size: 0.76rem;
		font-weight: 650;
	}

	.summary-detail-parts {
		display: flex;
		flex-wrap: wrap;
		gap: 0.08rem 0.42rem;
		align-items: baseline;
		line-height: 1.28;
	}

	.summary-detail-parts span {
		white-space: nowrap;
	}

	.summary-card > small.good-detail {
		color: #167044;
	}

	.summary-card > small.caution-detail {
		color: #9c3929;
	}

	.workspace-grid {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: 0.86rem;
		align-items: start;
		min-width: 0;
		max-width: 100%;
		box-sizing: border-box;
	}

	.panel {
		display: grid;
		gap: 0.8rem;
		padding: 1rem;
	}

	.section-head {
		display: flex;
		align-items: start;
		justify-content: space-between;
		gap: 0.8rem;
	}

	.section-head div {
		display: grid;
		gap: 0.18rem;
	}

	.section-head a {
		color: #4d4036;
		font-size: 0.82rem;
		font-weight: 760;
		text-decoration: none;
	}

	.section-head a:hover {
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.protocol-list {
		display: grid;
		border-top: 1px solid rgba(23, 32, 42, 0.08);
	}

	.protocol-list-more {
		display: inline-flex;
		align-items: center;
		justify-content: flex-start;
		min-height: 2.5rem;
		padding: 0.1rem 0;
		color: #4d4036;
		font-size: 0.84rem;
		font-weight: 780;
		text-decoration: none;
	}

	.protocol-list-more:hover {
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.empty-state {
		color: var(--fw-muted);
		font-size: 0.82rem;
		line-height: 1.45;
	}

	.empty-state {
		padding: 0.9rem 0;
	}

	@media (max-width: 980px) {
		.overview-panel,
		.workspace-grid {
			grid-template-columns: 1fr;
		}

		.summary-grid {
			grid-template-columns: repeat(2, minmax(0, 1fr));
		}
	}

	@media (max-width: 640px) {
		.home-shell {
			width: auto;
			margin: 0 0.5rem;
			padding-top: 0.8rem;
		}

		.summary-grid {
			grid-template-columns: repeat(2, minmax(0, 1fr));
			gap: 0.5rem;
		}

		.alert-preview {
			grid-template-columns: 1fr;
			gap: 0.44rem;
		}

		.summary-card {
			padding: 0.72rem;
		}

		.summary-card strong {
			font-size: 1.06rem;
		}

		.overview-copy {
			min-height: 0;
			padding: 1rem;
		}

		h1 {
			max-width: none;
			font-size: 2.3rem;
		}

		.section-head {
			display: grid;
		}
	}

	@media (max-width: 350px) {
		.summary-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
