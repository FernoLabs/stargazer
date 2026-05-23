<script lang="ts">
	import { explorerAddressUrl, explorerIdlUrl } from '$lib/explorer';
	import { formatMetricValue, formatTimestamp, readableInstructionName, shortAddress } from '$lib/format';
	import type { ActionData, PageData } from './$types';
	import type { DefiProtocolDescriptor, MetricAlertPreset } from '$lib/types';

	let { data, form }: { data: PageData; form?: ActionData } = $props();

	let activeTab = $state<'builder' | 'rules' | 'catalog'>('builder');
	let telegramScope = $state<'defi_watch' | 'program_updates' | 'multisig_activity'>('defi_watch');
	let telegramProtocolSlug = $state('');
	let telegramKnownProgramId = $state('');
	let telegramTargetAddress = $state('');
	let telegramKnownDefiOnly = $state(false);
	let selectedMetricPresetKey = $state('');

	function protocolBySlug(slug: string) {
		return data.defiProtocols.find((protocol) => protocol.slug === slug) ?? null;
	}

	function protocolCoverageCopy(protocol: DefiProtocolDescriptor) {
		if (protocol.coverage === 'deep_decode') {
			return `${protocol.catalog_instruction_count} decoded instructions`;
		}

		if (protocol.coverage === 'admin_signals') {
			return 'Admin and IDL signals';
		}

		return 'Upgrade and address watch';
	}

	function watchScopeLabel(scope: string) {
		if (scope === 'program_updates') return 'Program updates';
		if (scope === 'multisig_activity') return 'Multisig activity';
		return 'DeFi watch';
	}

	function metricPresetKey(preset: MetricAlertPreset) {
		return [
			preset.protocol_slug,
			preset.instruction_name,
			preset.metric_key,
			preset.subject_address ?? ''
		].join('|');
	}

	function selectedMetricPreset() {
		return data.metricAlertPresets.find((preset) => metricPresetKey(preset) === selectedMetricPresetKey) ?? null;
	}

	function resetProgramFilters() {
		telegramProtocolSlug = '';
		telegramKnownProgramId = '';
		telegramTargetAddress = '';
	}

	function handleProtocolChange() {
		telegramKnownProgramId = '';
		const protocol = protocolBySlug(telegramProtocolSlug);
		if (
			protocol &&
			telegramTargetAddress &&
			!protocol.programs.some((program) => program.program_id === telegramTargetAddress)
		) {
			return;
		}
		telegramTargetAddress = '';
	}

	function useKnownProgram() {
		if (telegramKnownProgramId) {
			telegramTargetAddress = telegramKnownProgramId;
		}
	}

	function selectedProtocol() {
		return protocolBySlug(telegramProtocolSlug);
	}

	$effect(() => {
		const firstPreset = data.metricAlertPresets[0];
		if (firstPreset && !selectedMetricPresetKey) {
			selectedMetricPresetKey = metricPresetKey(firstPreset);
		}
	});
</script>

<svelte:head>
	<title>FireWatch Alerts</title>
</svelte:head>

<div class="page-shell">
		<section class="hero">
			<div class="hero-copy">
				<h1>Build lean Telegram alerts around the protocols you actually use.</h1>
			<p class="lede">
				Start with one DeFi protocol rule. Add program update, IDL-only, multisig, or metric guards
				when you need more precision.
			</p>

			{#if form?.message}
				<p class="flash success">{form.message}</p>
			{/if}
			{#if form?.error}
				<p class="flash error">{form.error}</p>
			{/if}
			{#if data.metricBackendError}
				<p class="flash error">Metric presets unavailable: {data.metricBackendError}</p>
			{/if}
			{#if !data.telegramEnabled}
				<p class="flash error">Telegram is disabled until <code>TELEGRAM_BOT_TOKEN</code> is configured on the backend.</p>
			{/if}
		</div>

		<div class="hero-stats">
			<article class="stat-card">
				<span>Active rules</span>
				<strong>{data.telegramSubscriptions.length}</strong>
			</article>
			<article class="stat-card">
				<span>Metric rules</span>
				<strong>{data.metricAlertRules.length}</strong>
			</article>
			<article class="stat-card">
				<span>Protocols</span>
				<strong>{data.protocolCount}</strong>
			</article>
			<article class="stat-card">
				<span>IDL tracked</span>
				<strong>{data.idlTrackedProgramCount}</strong>
			</article>
		</div>
	</section>

	<section class="panel tabs-panel">
		<div class="tab-row" role="tablist" aria-label="Alert sections">
			<button class:active={activeTab === 'builder'} class="tab-button" type="button" onclick={() => (activeTab = 'builder')}>
				Create rule
			</button>
			<button class:active={activeTab === 'rules'} class="tab-button" type="button" onclick={() => (activeTab = 'rules')}>
				Current rules
			</button>
			<button class:active={activeTab === 'catalog'} class="tab-button" type="button" onclick={() => (activeTab = 'catalog')}>
				Catalog
			</button>
		</div>
	</section>

	{#if activeTab === 'builder'}
		<section class="builder-grid">
			<article class="panel">
				<div class="panel-head">
					<h2>Create Telegram rule</h2>
					<p>Protocol-first by default. Keep the scope narrow and the signal high.</p>
				</div>

				<div class="preset-row">
					<button class:active={telegramScope === 'defi_watch'} class="preset-button" type="button" onclick={() => {
						telegramScope = 'defi_watch';
						telegramKnownDefiOnly = false;
					}}>
						DeFi watch
					</button>
					<button class:active={telegramScope === 'program_updates'} class="preset-button" type="button" onclick={() => {
						telegramScope = 'program_updates';
						resetProgramFilters();
					}}>
						Program updates
					</button>
					<button class:active={telegramScope === 'multisig_activity'} class="preset-button" type="button" onclick={() => {
						telegramScope = 'multisig_activity';
						resetProgramFilters();
					}}>
						Multisig
					</button>
				</div>

				<form class="rule-form" method="POST" action="?/create_telegram_subscription">
					<input name="telegram_scope" type="hidden" value={telegramScope} />

					<div class="field-grid">
						<label>
							<span>Rule label</span>
							<input name="telegram_label" type="text" placeholder="Jupiter + Kamino ops" required />
						</label>

						<label>
							<span>Telegram chat id</span>
							<input name="telegram_chat_id" type="text" placeholder="-1001234567890 or @channelname" required />
						</label>
					</div>

					<label>
						<span>Topic thread id</span>
						<input name="telegram_thread_id" type="text" placeholder="Optional forum topic id" />
					</label>

					{#if telegramScope === 'defi_watch'}
						<label>
							<span>Protocol</span>
							<select bind:value={telegramProtocolSlug} onchange={handleProtocolChange}>
								<option value="">All curated protocols</option>
								{#each data.defiProtocols as protocol}
									<option value={protocol.slug}>{protocol.name}</option>
								{/each}
							</select>
						</label>

						{#if selectedProtocol()}
							{@const protocol = selectedProtocol()!}
							<input name="telegram_required_tags" type="hidden" value={`protocol:${protocol.slug}`} />

							<label>
								<span>Official program</span>
								<select bind:value={telegramKnownProgramId} onchange={useKnownProgram}>
									<option value="">All {protocol.name} programs</option>
									{#each protocol.programs as program}
										<option value={program.program_id}>{program.label} · {shortAddress(program.program_id)}</option>
									{/each}
								</select>
							</label>

							<div class="coverage-note">
								<span class={`coverage coverage-${protocol.coverage}`}>{protocol.coverage.replace('_', ' ')}</span>
								<p>{protocolCoverageCopy(protocol)}</p>
							</div>

							<div class="check-grid">
								<label class="check-row">
									<input name="telegram_required_tags" type="checkbox" value="impact:high" checked />
									<div>
										<strong>High-impact only</strong>
										<p>Recommended for low-noise protocol risk alerts.</p>
									</div>
								</label>

								<label class="check-row">
									<input name="telegram_required_tags" type="checkbox" value="signal:metric_anomaly" />
									<div>
										<strong>Metric anomalies only</strong>
										<p>Only alert on saved metric threshold breaches.</p>
									</div>
								</label>

								{#if protocol.unknown_alerts}
									<label class="check-row">
										<input name="telegram_required_tags" type="checkbox" value="signal:unknown_instruction" />
										<div>
											<strong>Unknown instructions only</strong>
											<p>Use this for urgent research mode.</p>
										</div>
									</label>
								{/if}
							</div>

							{#if protocol.admin_instruction_filters.length > 0}
								<div class="tag-grid">
									{#each protocol.high_impact_instruction_filters.slice(0, 8) as instruction}
										<label class="tag-check">
											<input name="telegram_required_tags" type="checkbox" value={`instruction:${instruction}`} />
											<span>{readableInstructionName(instruction)}</span>
										</label>
									{/each}
								</div>
							{/if}
						{/if}
					{/if}

					{#if telegramScope === 'program_updates'}
						<label class="check-row">
							<input bind:checked={telegramKnownDefiOnly} type="checkbox" onchange={() => {
								if (!telegramKnownDefiOnly) {
									resetProgramFilters();
								}
							}} />
							<div>
								<strong>Known FireWatch DeFi programs only</strong>
								<p>Restrict updates to the curated protocol catalog.</p>
							</div>
						</label>

						{#if telegramKnownDefiOnly}
							<input name="telegram_required_tags" type="hidden" value="watch:defi" />

							<label>
								<span>Protocol</span>
								<select bind:value={telegramProtocolSlug} onchange={handleProtocolChange}>
									<option value="">All FireWatch-known DeFi protocols</option>
									{#each data.defiProtocols as protocol}
										<option value={protocol.slug}>{protocol.name}</option>
									{/each}
								</select>
							</label>

							{#if selectedProtocol()}
								{@const protocol = selectedProtocol()!}
								<input name="telegram_required_tags" type="hidden" value={`protocol:${protocol.slug}`} />
								<label>
									<span>Official program</span>
									<select bind:value={telegramKnownProgramId} onchange={useKnownProgram}>
										<option value="">All {protocol.name} programs</option>
										{#each protocol.programs as program}
											<option value={program.program_id}>{program.label} · {shortAddress(program.program_id)}</option>
										{/each}
									</select>
								</label>
							{/if}
						{/if}

						<label>
							<span>Target address</span>
							<input bind:value={telegramTargetAddress} name="telegram_target_address" type="text" placeholder="Optional: one program address" />
						</label>

						<div class="check-grid">
							<label class="check-row">
								<input name="telegram_required_tags" type="checkbox" value="signal:idl_update" />
								<div>
									<strong>IDL changes only</strong>
									<p>Only alert on canonical on-chain IDL metadata updates.</p>
								</div>
							</label>
						</div>
					{/if}

					{#if telegramScope === 'multisig_activity'}
						<label>
							<span>Target address</span>
							<input bind:value={telegramTargetAddress} name="telegram_target_address" type="text" placeholder="Optional: one Squads multisig address" />
						</label>
					{/if}

					{#if telegramScope === 'defi_watch'}
						<input name="telegram_target_address" type="hidden" value={telegramTargetAddress} />
					{/if}

					<div class="form-actions">
						<button class="primary-button" type="submit" disabled={!data.telegramEnabled}>
							Save Telegram rule
						</button>
						<p>Rules apply only to new matching events.</p>
					</div>
				</form>
			</article>

			<aside class="side-stack">
				<article class="panel compact-panel">
					<div class="panel-head">
						<h2>Recommended default</h2>
						<p>One protocol rule per platform you actively use.</p>
					</div>
					<ul class="bullet-list">
						<li>Use <code>DeFi watch</code> for protocol-level risk.</li>
						<li>Add <code>High-impact only</code> to keep Telegram lean.</li>
						<li>Add a metric rule only when one known instruction still matters operationally.</li>
					</ul>
				</article>

				<article class="panel compact-panel">
					<div class="panel-head">
						<h2>Bot setup</h2>
						<p>Already configured on the backend.</p>
					</div>
					<ul class="bullet-list">
						<li>Add the bot to the target chat or channel.</li>
						<li>Send <code>/help</code> to confirm it responds.</li>
						<li>Use <code>/watch</code> in Telegram for quick subscriptions.</li>
					</ul>
				</article>
			</aside>
		</section>

		<section class="panel">
			<div class="panel-head">
				<h2>Metric guardrails</h2>
				<p>Optional thresholds for common protocol instructions that matter only when the value is abnormal.</p>
			</div>

			{#if data.metricAlertPresets.length === 0}
				<p class="empty-state">No recent metric presets yet. Let the watcher run and this section will populate from live traffic.</p>
			{:else}
				<form class="metric-form" method="POST" action="?/create_metric_alert_rule">
					<label>
						<span>Observed metric</span>
						<select bind:value={selectedMetricPresetKey}>
							{#each data.metricAlertPresets as preset}
								<option value={metricPresetKey(preset)}>
									{preset.protocol_name} · {readableInstructionName(preset.instruction_name)} · {preset.metric_label}
								</option>
							{/each}
						</select>
					</label>

					{#if selectedMetricPreset()}
						{@const preset = selectedMetricPreset()!}
						<input type="hidden" name="metric_protocol_slug" value={preset.protocol_slug} />
						<input type="hidden" name="metric_instruction_name" value={preset.instruction_name} />
						<input type="hidden" name="metric_key" value={preset.metric_key} />
						<input type="hidden" name="metric_subject_address" value={preset.subject_address ?? ''} />

						<div class="field-grid">
							<label>
								<span>Rule label</span>
								<input name="metric_rule_label" type="text" value={`${preset.protocol_name} ${readableInstructionName(preset.instruction_name)} ${preset.metric_label}`} required />
							</label>
							<label>
								<span>Subject</span>
								<input type="text" value={preset.subject_address ?? 'All observed subjects'} readonly />
							</label>
						</div>

						<div class="field-grid">
							<label>
								<span>Minimum value</span>
								<input name="metric_min_value" type="number" step="any" value={preset.suggested_min_value ?? ''} />
							</label>
							<label>
								<span>Maximum value</span>
								<input name="metric_max_value" type="number" step="any" value={preset.suggested_max_value ?? ''} />
							</label>
						</div>

						<p class="inline-note">
							Latest value: <strong>{formatMetricValue(preset.latest_value)}</strong> from slot {preset.latest_slot}
						</p>
					{/if}

					<div class="form-actions">
						<button class="primary-button" type="submit">Save metric rule</button>
					</div>
				</form>
			{/if}
		</section>
	{/if}

	{#if activeTab === 'rules'}
		<section class="rule-grid">
			<article class="panel">
				<div class="panel-head">
					<h2>Telegram rules</h2>
					<p>Delete anything noisy before widening coverage.</p>
				</div>

				{#if data.telegramSubscriptions.length === 0}
					<p class="empty-state">No Telegram rules yet.</p>
				{:else}
					<div class="list-grid">
						{#each data.telegramSubscriptions as subscription}
							<article class="list-card">
								<div class="list-head">
									<div>
										<strong>{subscription.label}</strong>
										<p>{watchScopeLabel(subscription.scope)} · {formatTimestamp(subscription.created_at)}</p>
									</div>
									<span class="coverage">{watchScopeLabel(subscription.scope)}</span>
								</div>

								<ul class="meta-list">
									<li><strong>Chat</strong><span>{subscription.chat_id}</span></li>
									<li><strong>Topic</strong><span>{subscription.thread_id ?? 'Default'}</span></li>
									<li>
										<strong>Target</strong>
										{#if subscription.target_address}
											<a href={explorerAddressUrl(subscription.target_address, data.explorerCluster)} rel="noreferrer" target="_blank">
												{shortAddress(subscription.target_address)}
											</a>
										{:else}
											<span>All matching targets</span>
										{/if}
									</li>
									<li>
										<strong>Tags</strong>
										<span>{subscription.required_tags.length > 0 ? subscription.required_tags.join(', ') : 'None'}</span>
									</li>
								</ul>

								<form method="POST" action="?/delete_telegram_subscription">
									<input type="hidden" name="id" value={subscription.id} />
									<button class="ghost-button" type="submit">Delete rule</button>
								</form>
							</article>
						{/each}
					</div>
				{/if}
			</article>

			<article class="panel">
				<div class="panel-head">
					<h2>Metric rules</h2>
					<p>Use these only for protocols where raw value changes matter operationally.</p>
				</div>

				{#if data.metricAlertRules.length === 0}
					<p class="empty-state">No metric rules yet.</p>
				{:else}
					<div class="list-grid">
						{#each data.metricAlertRules as rule}
							<article class="list-card">
								<div class="list-head">
									<div>
										<strong>{rule.label}</strong>
										<p>{rule.protocol_name} · {readableInstructionName(rule.instruction_name)}</p>
									</div>
									<span class="coverage">Metric</span>
								</div>

								<ul class="meta-list">
									<li><strong>Metric</strong><span>{rule.metric_label}</span></li>
									<li><strong>Min</strong><span>{formatMetricValue(rule.min_value)}</span></li>
									<li><strong>Max</strong><span>{formatMetricValue(rule.max_value)}</span></li>
								</ul>

								<form method="POST" action="?/delete_metric_alert_rule">
									<input type="hidden" name="id" value={rule.id} />
									<button class="ghost-button" type="submit">Delete rule</button>
								</form>
							</article>
						{/each}
					</div>
				{/if}
			</article>
		</section>
	{/if}

	{#if activeTab === 'catalog'}
		<section class="panel">
			<div class="panel-head">
				<h2>Protocol catalog</h2>
				<p>Every tracked program shows its explorer page and canonical on-chain IDL metadata.</p>
			</div>

			<div class="catalog-grid">
				{#each data.defiProtocols as protocol}
					<article class="catalog-card">
						<div class="list-head">
							<div>
								<strong>{protocol.name}</strong>
								<p>{protocolCoverageCopy(protocol)}</p>
							</div>
							<span class={`coverage coverage-${protocol.coverage}`}>{protocol.coverage.replace('_', ' ')}</span>
						</div>

						<ul class="program-list">
							{#each protocol.programs as program}
								<li>
									<a href={explorerAddressUrl(program.program_id, data.explorerCluster)} rel="noreferrer" target="_blank">
										{program.label} · {shortAddress(program.program_id)}
									</a>
									<div class="program-links">
										<a href={explorerIdlUrl(program.program_id, data.explorerCluster)} rel="noreferrer" target="_blank">
											Explorer IDL
										</a>
										<a href={explorerAddressUrl(program.idl_metadata_address, data.explorerCluster)} rel="noreferrer" target="_blank">
											IDL PDA
										</a>
									</div>
								</li>
							{/each}
						</ul>
					</article>
				{/each}
			</div>
		</section>
	{/if}
</div>

<style>
	.page-shell {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem 0 2.4rem;
		display: grid;
		gap: 0.8rem;
	}

	.hero,
	.hero-stats,
	.builder-grid,
	.rule-grid,
	.catalog-grid,
	.list-grid,
	.field-grid,
	.check-grid,
	.tag-grid {
		display: grid;
		gap: 1rem;
	}

	.hero {
		grid-template-columns: minmax(0, 1.25fr) minmax(320px, 0.9fr);
	}

	.hero-stats {
		grid-template-columns: repeat(2, minmax(0, 1fr));
	}

	.builder-grid {
		grid-template-columns: minmax(0, 1.2fr) minmax(280px, 0.8fr);
	}

	.rule-grid {
		grid-template-columns: repeat(2, minmax(0, 1fr));
	}

	.catalog-grid {
		grid-template-columns: repeat(3, minmax(0, 1fr));
	}

	.field-grid {
		grid-template-columns: repeat(2, minmax(0, 1fr));
	}

	.check-grid,
	.list-grid {
		grid-template-columns: 1fr;
	}

	.tag-grid {
		grid-template-columns: repeat(3, minmax(0, 1fr));
	}

	.hero-copy,
	.stat-card,
	.panel,
	.list-card,
	.catalog-card {
		background: #fff;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 10px;
	}

	.hero-copy,
	.panel {
		padding: 1.4rem;
	}

	.stat-card,
	.list-card,
	.catalog-card {
		padding: 1.1rem;
	}

	h1,
	h2,
	strong {
		margin: 0;
	}

	h1 {
		font-size: 2.25rem;
		line-height: 1.02;
		letter-spacing: 0;
		max-width: 14ch;
		overflow-wrap: anywhere;
	}

	.lede,
	.panel-head p,
	.list-head p,
	.empty-state,
	.meta-list span,
	.meta-list a,
	.check-row p,
	.coverage-note p,
	.inline-note {
		color: #5f6c79;
	}

	.lede {
		margin: 1rem 0 0;
		line-height: 1.65;
		max-width: 56ch;
	}

	.panel-head {
		margin-bottom: 0.95rem;
	}

	.flash {
		margin-top: 1rem;
		padding: 0.78rem 0.9rem;
		border-radius: 8px;
		font-size: 0.92rem;
		font-weight: 600;
	}

	.flash.success {
		background: rgba(61, 140, 99, 0.14);
		color: #216d4d;
	}

	.flash.error {
		background: rgba(191, 74, 57, 0.14);
		color: #9c3929;
	}

	.tabs-panel {
		padding: 0.5rem 0.8rem;
	}

	.tab-row,
	.preset-row,
	.program-links {
		display: flex;
		flex-wrap: wrap;
		gap: 0.6rem;
	}

	.tab-button,
	.preset-button,
	.primary-button,
	.ghost-button,
	.tag-check,
	.coverage {
		border-radius: 8px;
		font: inherit;
	}

	.tab-button,
	.preset-button,
	.ghost-button {
		padding: 0.58rem 0.8rem;
		border: 1px solid rgba(23, 32, 42, 0.09);
		background: #fff;
		color: #556473;
		cursor: pointer;
		font-weight: 600;
	}

	.tab-button.active,
	.preset-button.active {
		background: rgba(255, 123, 54, 0.12);
		color: #bd4a17;
	}

	.rule-form,
	.metric-form,
	label {
		display: grid;
		gap: 0.8rem;
	}

	label span {
		font-size: 0.88rem;
		font-weight: 700;
	}

	input,
	select {
		width: 100%;
		min-width: 0;
		box-sizing: border-box;
		padding: 0.72rem 0.82rem;
		border-radius: 8px;
		border: 1px solid rgba(23, 32, 42, 0.12);
		background: #fff;
		font: inherit;
	}

	.check-row {
		grid-template-columns: auto 1fr;
		align-items: start;
		gap: 0.75rem;
		padding: 0.92rem 1rem;
		border-radius: 8px;
		background: #fbfaf8;
		border: 1px solid rgba(23, 32, 42, 0.08);
	}

	.check-row input {
		width: auto;
		margin-top: 0.15rem;
	}

	.coverage-note,
	.inline-note {
		padding: 0.85rem 0.95rem;
		border-radius: 8px;
		background: #fbfaf8;
	}

	.coverage {
		display: inline-flex;
		padding: 0.32rem 0.6rem;
		background: rgba(23, 32, 42, 0.06);
		color: #526170;
		font-size: 0.76rem;
		font-weight: 700;
	}

	.coverage-deep_decode {
		background: rgba(61, 140, 99, 0.14);
		color: #1f7c56;
	}

	.coverage-admin_signals {
		background: rgba(255, 123, 54, 0.14);
		color: #bd4a17;
	}

	.tag-check {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
		padding: 0.7rem 0.82rem;
		background: #fbfaf8;
		border: 1px solid rgba(23, 32, 42, 0.08);
	}

	.tag-check input {
		width: auto;
	}

	.form-actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		margin-top: 0.5rem;
	}

	.primary-button {
		padding: 0.86rem 1.05rem;
		border: 1px solid #bd4a17;
		background: #bd4a17;
		color: white;
		cursor: pointer;
		font-weight: 700;
	}

	.primary-button:disabled {
		cursor: not-allowed;
		opacity: 0.5;
	}

	.side-stack {
		display: grid;
		gap: 1rem;
	}

	.compact-panel {
		height: fit-content;
	}

	.bullet-list,
	.program-list,
	.meta-list {
		margin: 0;
		padding-left: 1.1rem;
		line-height: 1.6;
	}

	.list-head {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: 1rem;
		margin-bottom: 0.9rem;
	}

	.meta-list li + li,
	.program-list li + li {
		margin-top: 0.55rem;
	}

	.program-links {
		margin-top: 0.25rem;
		font-size: 0.86rem;
	}

	.program-links a {
		text-decoration: none;
		color: #667788;
	}

	@media (max-width: 980px) {
		.hero,
		.builder-grid,
		.rule-grid,
		.catalog-grid,
		.field-grid,
		.tag-grid {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 720px) {
		.page-shell {
			width: min(100vw - 1rem, var(--fw-page-max));
			padding-top: 0.8rem;
		}

		h1 {
			font-size: 1.9rem;
		}
	}
</style>
