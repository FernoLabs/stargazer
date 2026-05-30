<script lang="ts">
	import {
		joinCommandLines,
		watchDefiCommand,
		watchProgramCommand,
		watchProtocolCommand
	} from '$lib/bot-commands';
	import { shortAddress } from '$lib/format';
	import {
		firewatchPlanById,
		firewatchPlans,
		planPriceLine,
		type FireWatchPlanId
	} from '$lib/monetization';
	import type { DefiProtocolDescriptor, ProtocolProgramSurface } from '$lib/types';
	import ProtocolIdentity from './ProtocolIdentity.svelte';

	type BuilderMode = 'defi' | 'program' | 'protocol' | 'wallet';
	type CopiedState = 'idle' | 'copied' | 'error';
	type PlanTier = FireWatchPlanId;
	type WalletTrigger = 'any' | 'amount' | 'ratio';

	type ProgramOption = {
		key: string;
		protocol: DefiProtocolDescriptor;
		program: ProtocolProgramSurface;
	};

	const deploymentPlans = firewatchPlans;

	let {
		protocols,
		botUsername = null,
		defaultPlan = 'watch',
		defaultMode = 'defi',
		showPlanPanel = true,
		heading = 'Telegram command builder',
		description = 'Copy the free DeFi command, or choose Custom Plan to configure filtered watches.'
	}: {
		protocols: DefiProtocolDescriptor[];
		botUsername?: string | null;
		defaultPlan?: PlanTier;
		defaultMode?: BuilderMode;
		showPlanPanel?: boolean;
		heading?: string;
		description?: string;
	} = $props();

	function initialMode() {
		return defaultMode;
	}

	function initialPlan() {
		return defaultPlan;
	}

	let mode = $state<BuilderMode>(initialMode());
	let programQuery = $state('');
	let selectedProgramId = $state('');
	let selectedProtocolSlug = $state('');
	let selectedProtocolSlugs = $state<string[]>([]);
	let solDomainLabel = $state('');
	let walletAddress = $state('');
	let walletTrigger = $state<WalletTrigger>('any');
	let walletAmount = $state('10000');
	let walletToken = $state('USDC');
	let walletRatio = $state('20');
	let copiedState = $state<CopiedState>('idle');
	let selectedPlan = $state<PlanTier>(initialPlan());

	function programOptions(): ProgramOption[] {
		return protocols.flatMap((protocol) =>
			protocol.programs.map((program) => ({
				key: `${protocol.slug}:${program.program_id}`,
				protocol,
				program
			}))
		);
	}

	function protocolOptions() {
		return protocols.filter(
			(protocol) =>
				protocol.programs.length > 0 ||
				protocol.sol_domains.length > 0 ||
				protocol.amount_triggers.length > 0
		);
	}

	function selectedProgram() {
		return programOptions().find((option) => option.program.program_id === selectedProgramId) ?? null;
	}

	function selectedProtocol() {
		return protocolOptions().find((protocol) => protocol.slug === selectedProtocolSlug) ?? null;
	}

	function selectedProtocols() {
		const selected = new Set(selectedProtocolSlugs);
		return protocolOptions().filter((protocol) => selected.has(protocol.slug));
	}

	function selectedDeploymentPlan() {
		return firewatchPlanById(selectedPlan);
	}

	function hasCustomAccess() {
		return selectedPlan !== 'watch';
	}

	function effectiveMode(): BuilderMode {
		return hasCustomAccess() ? mode : 'defi';
	}

	function accessNote() {
		if (selectedPlan === 'watch') {
			return 'Free users can copy only /watch defi. Custom protocol, program, wallet, and filtered rules are saved through the Telegram bot after Custom Plan setup.';
		}

		if (selectedPlan === 'paid') {
			return 'Send the generated command in Telegram. If this Telegram user does not have Custom Plan access yet, the bot saves the monitor as pending and replies with a payment link.';
		}

		return 'Company response adds reviewed signals, security review, and a Smart Account action path after policy approval.';
	}

	function canCopyCommand() {
		return Boolean(command());
	}

	function botHref() {
		return botUsername ? `https://t.me/${botUsername}` : null;
	}

	function telegramSteps() {
		if (effectiveMode() === 'defi') {
			return ['Add the bot to a chat', 'Send /watch defi', 'Use /list to confirm the rule'];
		}

		return [
			'Send this command to the bot',
			'The bot saves the monitor',
			'Pay from the link if access is missing'
		];
	}

	function telegramActionLabel() {
		if (botHref()) return effectiveMode() === 'defi' ? 'Open bot' : 'Open Telegram';
		return 'Command reference';
	}

	function telegramActionHref() {
		return botHref() ?? '/docs#commands';
	}

	function planReferenceHref() {
		if (selectedPlan === 'paid') return '/pricing#custom-plan';
		if (selectedPlan === 'pro') return '/pricing#pro-response';
		return selectedDeploymentPlan().ctaHref;
	}

	function planReferenceLabel() {
		if (selectedPlan === 'paid') return 'Pricing';
		if (selectedPlan === 'pro') return 'Company response';
		return selectedDeploymentPlan().ctaLabel;
	}

	function currentProtocol() {
		if (effectiveMode() === 'defi' || effectiveMode() === 'wallet') return null;
		return mode === 'protocol' ? selectedProtocol() : selectedProgram()?.protocol ?? null;
	}

	function currentProtocols() {
		if (effectiveMode() !== 'protocol') return [];
		const protocols = selectedProtocols();
		return protocols.length ? protocols : selectedProtocol() ? [selectedProtocol()!] : [];
	}

	function visiblePrograms() {
		const query = programQuery.trim().toLowerCase();
		const options = programOptions();

		if (!query) return options.slice(0, 14);

		return options
			.filter((option) => {
				const haystack = [
					option.protocol.name,
					option.protocol.slug,
					option.program.label,
					option.program.program_id
				]
					.join(' ')
					.toLowerCase();
				return haystack.includes(query);
			})
			.slice(0, 18);
	}

	function normalizedSolDomain() {
		const raw = solDomainLabel.trim().toLowerCase().replace(/^@/, '');
		if (!raw) return '';
		return raw.endsWith('.sol') ? raw : `${raw}.sol`;
	}

	function command() {
		if (effectiveMode() === 'defi') {
			return watchDefiCommand();
		}

		if (mode === 'wallet') {
			return walletDraftCommand();
		}

		if (mode === 'protocol') {
			const protocols = currentProtocols();
			return protocols.length
				? joinCommandLines(protocols.map((protocol) => watchProtocolCommand(protocol.slug)))
				: '';
		}

		const option = selectedProgram();
		return option ? watchProgramCommand(option.program.program_id) : '';
	}

	function walletDraftCommand() {
		const address = walletAddress.trim();
		if (!address) return '';

		if (walletTrigger === 'amount') {
			const token = walletToken.trim().toUpperCase() || 'TOKEN';
			const amount = walletAmount.trim() || '0';
			return `/watch wallet ${address} amount >= ${amount} ${token}`;
		}

		if (walletTrigger === 'ratio') {
			const ratio = walletRatio.trim() || '0';
			return `/watch wallet ${address} moved >= ${ratio}%`;
		}

		return `/watch wallet ${address} any-transfer`;
	}

	function previewTitle() {
		if (effectiveMode() === 'defi') {
			return 'DeFi alert';
		}

		const domain = normalizedSolDomain();
		if (domain) return domain;

		if (mode === 'wallet') {
			return walletAddress.trim() ? shortAddress(walletAddress.trim()) : 'Paste a wallet';
		}

		if (mode === 'protocol') {
			const protocols = currentProtocols();
			if (protocols.length > 1) return `${protocols.length} protocols`;
			return protocols[0]?.name ?? 'Select protocols';
		}

		const option = selectedProgram();
		return option ? `${option.protocol.name} · ${option.program.label}` : 'Select a program';
	}

	function previewSubtitle() {
		if (effectiveMode() === 'defi') {
			return 'Unfiltered FireWatch DeFi feed';
		}

		if (mode === 'wallet') {
			if (walletTrigger === 'amount') {
				return `Token transfers at or above ${walletAmount || '0'} ${(walletToken || 'TOKEN').toUpperCase()}`;
			}

			if (walletTrigger === 'ratio') {
				return `Fund moves at or above ${walletRatio || '0'}% of wallet balance`;
			}

			return 'Any SOL or token transfer';
		}

		if (mode === 'protocol') {
			const protocols = currentProtocols();
			if (protocols.length > 1) {
				const programCount = protocols.reduce((count, protocol) => count + protocol.programs.length, 0);
				const solDomainCount = protocols.reduce(
					(count, protocol) => count + protocol.sol_domains.length,
					0
				);
				return `${programCount} programs${solDomainCount ? ` · ${solDomainCount} .sol domains` : ''}`;
			}
			const protocol = protocols[0];
			if (!protocol) return 'Protocol bundle';
			const parts = [
				protocol.programs.length ? `${protocol.programs.length} programs` : '',
				protocol.sol_domains.length ? `${protocol.sol_domains.length} .sol domains` : '',
				protocol.amount_triggers.length ? `${protocol.amount_triggers.length} triggers` : ''
			].filter(Boolean);
			return parts.length ? parts.join(' · ') : 'Protocol rule';
		}

		const option = selectedProgram();
		return option ? `${option.program.label} · ${shortAddress(option.program.program_id)}` : 'Program rule';
	}

	function catalogDomains() {
		const domains =
			effectiveMode() === 'protocol'
				? currentProtocols().flatMap((protocol) => protocol.sol_domains)
				: (currentProtocol()?.sol_domains ?? []);
		return Array.from(new Set(domains)).slice(0, 8);
	}

	function chooseProgram(option: ProgramOption) {
		if (!hasCustomAccess()) return;
		mode = 'program';
		selectedProgramId = option.program.program_id;
		selectedProtocolSlug = option.protocol.slug;
		copiedState = 'idle';
	}

	function chooseProtocol(slug: string) {
		if (!hasCustomAccess()) return;
		mode = 'protocol';
		selectedProtocolSlug = slug;
		selectedProtocolSlugs = selectedProtocolSlugs.includes(slug)
			? selectedProtocolSlugs.filter((selected) => selected !== slug)
			: [...selectedProtocolSlugs, slug];
		copiedState = 'idle';
	}

	async function copyCommand() {
		const text = command();
		if (!text || !canCopyCommand()) return;

		try {
			await navigator.clipboard.writeText(text);
			copiedState = 'copied';
		} catch {
			copiedState = 'error';
		}
	}

	$effect(() => {
		if (!hasCustomAccess() && mode !== 'defi') {
			mode = 'defi';
		}

		const programs = programOptions();
		if (!selectedProgramId && programs.length > 0) {
			selectedProgramId = programs[0].program.program_id;
		}

		if (
			selectedProgramId &&
			programs.length > 0 &&
			!programs.some((option) => option.program.program_id === selectedProgramId)
		) {
			selectedProgramId = programs[0].program.program_id;
		}

		const protocolList = protocolOptions();
		if (!selectedProtocolSlug && protocolList.length > 0) {
			selectedProtocolSlug = selectedProgram()?.protocol.slug ?? protocolList[0].slug;
		}

		if (
			selectedProtocolSlug &&
			protocolList.length > 0 &&
			!protocolList.some((protocol) => protocol.slug === selectedProtocolSlug)
		) {
			selectedProtocolSlug = protocolList[0].slug;
		}

		if (protocolList.length > 0) {
			const availableSlugs = new Set(protocolList.map((protocol) => protocol.slug));
			selectedProtocolSlugs = selectedProtocolSlugs.filter((slug) => availableSlugs.has(slug));
			if (selectedProtocolSlugs.length === 0 && selectedProtocolSlug) {
				selectedProtocolSlugs = [selectedProtocolSlug];
			}
		}
	});
</script>

<div class="builder">
	{#if showPlanPanel}
		<section class="plan-panel" aria-label="Choose your FireWatch plan">
			<div class="builder-head">
				<div>
					<h2>Choose access</h2>
					<p>Free users get the unfiltered DeFi feed. Custom Plan access unlocks monitor customization.</p>
				</div>
			</div>

			<div class="plan-grid" role="radiogroup" aria-label="FireWatch plan">
				{#each deploymentPlans as plan}
					<button
						aria-checked={selectedPlan === plan.id}
						class:active={selectedPlan === plan.id}
						class="plan-card"
						role="radio"
						type="button"
						onclick={() => (selectedPlan = plan.id)}
					>
						<span class="plan-head">
							<span>
								<strong>{plan.name}</strong>
								<span>{plan.description}</span>
							</span>
							<span class="plan-price">
								<strong>{plan.price}</strong>
								<span>{plan.priceNote}</span>
							</span>
						</span>

						<span class="plan-details">
							{#each plan.features as detail}
								<span>{detail}</span>
							{/each}
						</span>
					</button>
				{/each}
			</div>
		</section>
	{/if}

	<div class="builder-main">
		<div class="builder-head">
			<div>
				<h2>{heading}</h2>
				<p>{description}</p>
			</div>
			{#if botUsername}
				<a class="bot-link" href={`https://t.me/${botUsername}`} rel="noreferrer" target="_blank">
					@{botUsername}
				</a>
			{/if}
		</div>

		<div class="mode-row" role="tablist" aria-label="Command target">
			<button class:active={effectiveMode() === 'defi'} type="button" onclick={() => (mode = 'defi')}>
				DeFi alert
			</button>
			<button
				class:active={hasCustomAccess() && mode === 'program'}
				type="button"
				disabled={!hasCustomAccess()}
				title={!hasCustomAccess() ? 'Custom Plan required' : undefined}
				onclick={() => (mode = 'program')}
			>
				Program
			</button>
			<button
				class:active={hasCustomAccess() && mode === 'protocol'}
				type="button"
				disabled={!hasCustomAccess()}
				title={!hasCustomAccess() ? 'Custom Plan required' : undefined}
				onclick={() => (mode = 'protocol')}
			>
				Protocol bundle
			</button>
			<button
				class:active={hasCustomAccess() && mode === 'wallet'}
				type="button"
				disabled={!hasCustomAccess()}
				title={!hasCustomAccess() ? 'Custom Plan required' : undefined}
				onclick={() => (mode = 'wallet')}
			>
				Wallet
			</button>
		</div>

		<p class="access-note">{accessNote()}</p>

		{#if effectiveMode() === 'defi'}
			<div class="free-feed">
				<strong>Unfiltered DeFi alert</strong>
				<p>
					Free access subscribes the chat to FireWatch DeFi alerts without protocol, program,
					wallet, or threshold filters.
				</p>
				<code>{watchDefiCommand()}</code>
			</div>
		{:else if effectiveMode() === 'program'}
			<label class="search-field">
				<span>Program</span>
				<input bind:value={programQuery} type="search" placeholder="Search protocol, label, or address" />
			</label>

			<div class="program-list" role="list" aria-label="Programs">
				{#each visiblePrograms() as option}
					<button
						class:active={selectedProgramId === option.program.program_id}
						class="program-option"
						type="button"
						onclick={() => chooseProgram(option)}
					>
						<span class="program-copy">
							<ProtocolIdentity slug={option.protocol.slug} label={option.protocol.name} size="sm" />
							<strong>{option.program.label}</strong>
							<code>{shortAddress(option.program.program_id)}</code>
						</span>
					</button>
				{/each}
			</div>
		{:else if effectiveMode() === 'protocol'}
			<div class="selection-summary">
				<strong>{currentProtocols().length} selected</strong>
				<span>Each selected protocol creates one Telegram watch command.</span>
			</div>
			<div class="protocol-list" role="list" aria-label="Protocols">
				{#each protocolOptions() as protocol}
					<button
						aria-pressed={selectedProtocolSlugs.includes(protocol.slug)}
						class:active={selectedProtocolSlugs.includes(protocol.slug)}
						class="protocol-option"
						type="button"
						onclick={() => chooseProtocol(protocol.slug)}
					>
						<ProtocolIdentity slug={protocol.slug} label={protocol.name} size="sm" />
						<span>
							{protocol.programs.length} programs
							{#if protocol.sol_domains.length > 0}
								· {protocol.sol_domains.length} .sol
							{/if}
						</span>
					</button>
				{/each}
			</div>
		{:else}
			<div class="wallet-form">
				<label class="search-field">
					<span>Wallet</span>
					<input bind:value={walletAddress} type="text" placeholder="Wallet address" />
				</label>

				<div class="trigger-grid" role="radiogroup" aria-label="Wallet trigger">
					<button
						class:active={walletTrigger === 'any'}
						class="trigger-button"
						type="button"
						onclick={() => (walletTrigger = 'any')}
					>
						<strong>Any transfer</strong>
						<span>SOL or token movement</span>
					</button>
					<button
						class:active={walletTrigger === 'amount'}
						class="trigger-button"
						type="button"
						onclick={() => (walletTrigger = 'amount')}
					>
						<strong>Amount</strong>
						<span>Only larger token moves</span>
					</button>
					<button
						class:active={walletTrigger === 'ratio'}
						class="trigger-button"
						type="button"
						onclick={() => (walletTrigger = 'ratio')}
					>
						<strong>Fund ratio</strong>
						<span>Percent of wallet moved</span>
					</button>
				</div>

				{#if walletTrigger === 'amount'}
					<div class="field-grid">
						<label class="search-field">
							<span>Minimum amount</span>
							<input bind:value={walletAmount} type="number" min="0" step="0.01" inputmode="decimal" />
						</label>
						<label class="search-field">
							<span>Token</span>
							<input bind:value={walletToken} type="text" placeholder="USDC" />
						</label>
					</div>
				{:else if walletTrigger === 'ratio'}
					<label class="search-field">
						<span>Minimum fund move</span>
						<div class="suffix-field">
							<input bind:value={walletRatio} type="number" min="0" max="100" step="0.1" inputmode="decimal" />
							<span>%</span>
						</div>
					</label>
				{/if}
			</div>
		{/if}

		{#if effectiveMode() !== 'defi'}
			<label class="search-field">
				<span>{effectiveMode() === 'wallet' ? 'Wallet label' : '.sol label'}</span>
				<input bind:value={solDomainLabel} type="text" placeholder="Optional: your-name.sol" />
			</label>

			{#if catalogDomains().length > 0}
				<div class="domain-row" aria-label="Catalog domains">
					{#each catalogDomains() as domain}
						<button type="button" onclick={() => (solDomainLabel = domain)}>{domain}</button>
					{/each}
				</div>
			{/if}
		{/if}
	</div>

	<aside class="preview-panel" aria-label="Command preview">
		<div class="preview-copy">
			<span>{effectiveMode() === 'defi' ? 'Send to bot' : 'Setup draft'}</span>
			<strong>{previewTitle()}</strong>
			<p>{previewSubtitle()}</p>
			<p class="plan-summary">
				{selectedDeploymentPlan().name} · {planPriceLine(selectedDeploymentPlan())}
			</p>
			<p>{accessNote()}</p>
		</div>

		<div class="command-box">
			<code>{command() || '/watch'}</code>
		</div>

		<ol class="telegram-steps">
			{#each telegramSteps() as step}
				<li>{step}</li>
			{/each}
		</ol>

		<div class="preview-actions">
			<button class="copy-button" type="button" onclick={copyCommand} disabled={!canCopyCommand()}>
				{effectiveMode() === 'defi' ? 'Copy' : 'Copy draft'}
			</button>
			<a class="setup-link" href={telegramActionHref()} rel={botHref() ? 'noreferrer' : undefined} target={botHref() ? '_blank' : undefined}>
				{telegramActionLabel()}
			</a>
			<a class="secondary-link" href={planReferenceHref()}>
				{planReferenceLabel()}
			</a>
			{#if copiedState === 'copied'}
				<p class="copy-state success">{effectiveMode() === 'defi' ? 'Copied.' : 'Draft copied.'}</p>
			{:else if copiedState === 'error'}
				<p class="copy-state error">Copy failed.</p>
			{/if}
		</div>
	</aside>
</div>

<style>
	.builder {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(280px, 0.46fr);
		gap: 1rem;
	}

	.plan-panel {
		grid-column: 1 / -1;
		display: grid;
		gap: 0.72rem;
	}

	.builder-main,
	.preview-panel,
	.builder-head,
	.program-copy,
	.preview-copy,
	.wallet-form,
	.free-feed {
		display: grid;
		gap: 0.68rem;
		min-width: 0;
	}

	.builder-head {
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: start;
	}

	h2,
	p,
	strong {
		margin: 0;
	}

	h2 {
		font-size: 1.08rem;
		line-height: 1.2;
		letter-spacing: 0;
	}

	.builder-head p,
	.preview-copy p,
	.protocol-option span,
	.copy-state,
	.access-note,
	.free-feed p,
	.plan-card,
	.plan-details span {
		color: #5f6c79;
	}

	.builder-head p,
	.preview-copy p,
	.plan-head > span:first-child > span {
		font-size: 0.88rem;
		line-height: 1.48;
	}

	.plan-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 0.52rem;
	}

	.plan-card {
		display: grid;
		gap: 0.66rem;
		width: 100%;
		min-height: 100%;
		padding: 0.82rem;
		border-radius: 8px;
		border: 1px solid rgba(23, 32, 42, 0.1);
		background: #fbfaf8;
		font: inherit;
		text-align: left;
		cursor: pointer;
	}

	.plan-card.active {
		border-color: rgba(189, 74, 23, 0.5);
		background: rgba(255, 123, 54, 0.1);
	}

	.plan-head {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 0.8rem;
	}

	.plan-head > span:first-child,
	.plan-price {
		display: grid;
		gap: 0.18rem;
		min-width: 0;
	}

	.plan-head strong {
		color: #17202a;
		font-size: 0.92rem;
		line-height: 1.2;
	}

	.plan-price {
		flex: 0 0 auto;
		text-align: right;
	}

	.plan-price strong {
		color: #7d3a18;
		font-size: 1rem;
	}

	.plan-price span {
		color: #62707d;
		font-size: 0.7rem;
		font-weight: 700;
	}

	.plan-details {
		display: grid;
		gap: 0.22rem;
		padding-top: 0.6rem;
		border-top: 1px solid rgba(23, 32, 42, 0.08);
	}

	.plan-details span {
		font-size: 0.78rem;
		line-height: 1.35;
	}

	.plan-summary {
		font-weight: 700;
		color: #7d3a18 !important;
	}

	.bot-link {
		color: #7b5b45;
		font-size: 0.84rem;
		font-weight: 700;
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.mode-row,
	.domain-row,
	.preview-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.46rem;
	}

	.mode-row button,
	.domain-row button,
	.copy-button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-height: 2.2rem;
		border-radius: 8px;
		border: 1px solid rgba(23, 32, 42, 0.12);
		background: #fff;
		color: #556473;
		font: inherit;
		font-size: 0.84rem;
		font-weight: 700;
		cursor: pointer;
	}

	.mode-row button {
		padding: 0 0.78rem;
	}

	.mode-row button.active,
	.copy-button {
		border-color: #bd4a17;
		background: #bd4a17;
		color: white;
	}

	.mode-row button:disabled {
		cursor: not-allowed;
		opacity: 0.52;
	}

	.access-note {
		margin: -0.1rem 0 0;
		padding: 0.64rem 0.72rem;
		border-radius: 8px;
		border: 1px solid rgba(23, 32, 42, 0.08);
		background: #fbfaf8;
		font-size: 0.84rem;
		line-height: 1.45;
	}

	.free-feed {
		padding: 0.78rem 0.82rem;
		border-radius: 8px;
		border: 1px solid rgba(23, 32, 42, 0.08);
		background: #fff;
	}

	.free-feed strong {
		font-size: 0.92rem;
	}

	.free-feed p {
		font-size: 0.86rem;
		line-height: 1.5;
	}

	.search-field {
		display: grid;
		gap: 0.32rem;
	}

	.search-field span,
	.preview-copy span {
		color: #62707d;
		font-size: 0.76rem;
		font-weight: 700;
	}

	input {
		width: 100%;
		min-height: 2.42rem;
		padding: 0.58rem 0.68rem;
		border-radius: 8px;
		border: 1px solid rgba(23, 32, 42, 0.12);
		background: #fff;
		color: #17202a;
		font: inherit;
		font-size: 0.9rem;
	}

	input:focus {
		outline: 2px solid rgba(189, 74, 23, 0.2);
		border-color: rgba(189, 74, 23, 0.55);
	}

	.program-list,
	.protocol-list {
		display: grid;
		gap: 0.42rem;
		max-height: 26rem;
		overflow: auto;
		padding-right: 0.12rem;
	}

	.selection-summary {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.8rem;
		padding: 0.62rem 0.72rem;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		background: #fbfaf8;
	}

	.selection-summary strong {
		font-size: 0.86rem;
	}

	.selection-summary span {
		color: #62707d;
		font-size: 0.78rem;
		font-weight: 700;
		line-height: 1.35;
		text-align: right;
	}

	.trigger-grid,
	.field-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 0.42rem;
	}

	.field-grid {
		grid-template-columns: repeat(2, minmax(0, 1fr));
	}

	.program-option,
	.protocol-option,
	.trigger-button {
		width: 100%;
		padding: 0.66rem 0.72rem;
		border-radius: 8px;
		border: 1px solid rgba(23, 32, 42, 0.08);
		background: #fbfaf8;
		color: inherit;
		text-align: left;
		font: inherit;
		cursor: pointer;
	}

	.program-option.active,
	.protocol-option.active,
	.trigger-button.active {
		border-color: rgba(189, 74, 23, 0.44);
		background: rgba(255, 123, 54, 0.1);
	}

	.program-copy {
		gap: 0.18rem;
	}

	.program-copy strong {
		font-size: 0.9rem;
	}

	.trigger-button {
		display: grid;
		gap: 0.16rem;
	}

	.trigger-button strong {
		font-size: 0.84rem;
	}

	.trigger-button span {
		color: #62707d;
		font-size: 0.76rem;
		font-weight: 600;
		line-height: 1.35;
	}

	.suffix-field {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		border-radius: 8px;
		border: 1px solid rgba(23, 32, 42, 0.12);
		background: #fff;
	}

	.suffix-field input {
		border: 0;
		background: transparent;
	}

	.suffix-field input:focus {
		outline: 0;
		border-color: transparent;
	}

	.suffix-field:focus-within {
		outline: 2px solid rgba(189, 74, 23, 0.2);
		border-color: rgba(189, 74, 23, 0.55);
	}

	.suffix-field > span {
		padding-right: 0.68rem;
		color: #62707d;
		font-size: 0.84rem;
		font-weight: 800;
	}

	code {
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
		font-size: 0.8rem;
		color: #63707c;
		overflow-wrap: anywhere;
	}

	.protocol-option {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		gap: 0.7rem;
	}

	.protocol-option span {
		font-size: 0.78rem;
		font-weight: 700;
		white-space: nowrap;
	}

	.domain-row button {
		min-height: 1.9rem;
		padding: 0 0.58rem;
		background: #fff;
		color: #6d5a4a;
		font-size: 0.78rem;
	}

	.preview-panel {
		align-content: start;
		padding: 0.88rem;
		border-radius: 8px;
		border: 1px solid rgba(23, 32, 42, 0.08);
		background: #fbfaf8;
	}

	.preview-copy {
		gap: 0.22rem;
	}

	.preview-copy strong {
		font-size: 1.05rem;
		overflow-wrap: anywhere;
	}

	.command-box {
		padding: 0.78rem 0.82rem;
		border-radius: 8px;
		border: 1px solid rgba(23, 32, 42, 0.1);
		background: #fff;
	}

	.command-box code {
		color: #17202a;
		font-size: 0.86rem;
	}

	.copy-button {
		padding: 0 0.9rem;
	}

	.setup-link {
		display: inline-flex;
		align-items: center;
		min-height: 2.2rem;
		color: #6d5a4a;
		font-size: 0.84rem;
		font-weight: 700;
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.secondary-link {
		display: inline-flex;
		align-items: center;
		min-height: 2.2rem;
		color: #62707d;
		font-size: 0.82rem;
		font-weight: 700;
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.telegram-steps {
		display: grid;
		gap: 0.28rem;
		margin: 0;
		padding-left: 1.1rem;
		color: #5f6c79;
		font-size: 0.84rem;
		line-height: 1.45;
	}

	.copy-button:disabled {
		cursor: not-allowed;
		opacity: 0.55;
	}

	.copy-state {
		align-self: center;
		font-size: 0.84rem;
		font-weight: 700;
	}

	.copy-state.success {
		color: #216d4d;
	}

	.copy-state.error {
		color: #9c3929;
	}

	@media (max-width: 900px) {
		.builder {
			grid-template-columns: 1fr;
		}

		.plan-grid {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 620px) {
		.builder-head,
		.protocol-option {
			grid-template-columns: 1fr;
		}

		.program-list,
		.protocol-list {
			max-height: 22rem;
		}

		.trigger-grid,
		.field-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
