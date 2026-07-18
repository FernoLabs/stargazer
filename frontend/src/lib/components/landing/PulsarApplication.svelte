<script lang="ts">
	import { resolve } from '$app/paths';

	type EvidenceState = 'Ready locally' | 'Draft' | 'In progress' | 'Required';

	type ApplicationEvidence = {
		id: string;
		artifact: string;
		detail: string;
		state: EvidenceState;
	};

	type Deliverable = {
		id: string;
		title: string;
		detail: string;
		acceptance: string;
		allocation: string;
	};

	type Tranche = {
		id: string;
		title: string;
		window: string;
		allocation: string;
		deliverables: Deliverable[];
	};

	const evidence: ApplicationEvidence[] = [
		{
			id: 'architecture',
			artifact: 'Architecture and research dossier',
			detail:
				'Ingestion boundaries, provider evaluation, a cited DeFi seed, control-plane models, and incident fixtures are documented.',
			state: 'Ready locally'
		},
		{
			id: 'coverage',
			artifact: 'Research coverage',
			detail:
				'36 selected deployments across seven protocol families, 38 resolved contract IDs, and seven verified incident ledgers.',
			state: 'Ready locally'
		},
		{
			id: 'repository',
			artifact: 'Reviewer repository',
			detail:
				'A writable Ferno Pulsar origin, reviewable history, green CI, security policy, and one-command setup must be public.',
			state: 'Required'
		},
		{
			id: 'poc',
			artifact: 'Reproducible Stellar vertical slice',
			detail:
				'One command must replay a real upgrade twice and produce one evidence-backed finding with zero duplicates.',
			state: 'In progress'
		},
		{
			id: 'demo',
			artifact: 'Demo recording',
			detail:
				'A two-to-three-minute walkthrough must connect the replay command to API, dashboard, and Telegram evidence.',
			state: 'Required'
		},
		{
			id: 'operators',
			artifact: 'Operator validation',
			detail:
				'Five interviews, three permissioned quotes, and two pilot or letter-of-intent statements must validate demand.',
			state: 'Required'
		},
		{
			id: 'delivery',
			artifact: 'Named delivery and cost basis',
			detail:
				'Every deliverable needs an owner, availability, date, role days, rate, acceptance URL, and reconciled cost.',
			state: 'Draft'
		}
	];

	const tranches: Tranche[] = [
		{
			id: 'mvp',
			title: 'Production MVP',
			window: 'Months 1–2',
			allocation: '$36k',
			deliverables: [
				{
					id: 'D1',
					title: 'Restart-safe ingestion',
					detail:
						'Process recent Mainnet ledgers through a source-independent boundary with continuity checks, atomic checkpoints, gap handling, and retained raw evidence.',
					acceptance:
						'A published range resumes after interruption; a deliberate gap, conflicting hash, and parser failure stop or surface without silent loss.',
					allocation: '$12k'
				},
				{
					id: 'D2',
					title: 'Contract and Wasm history',
					detail:
						'Persist contract creation, Wasm uploads, version relationships, interfaces, and upgrades confirmed by event and instance diff.',
					acceptance:
						'One real upgrade retains old and new executable evidence; replaying twice produces identical facts and zero duplicate findings.',
					allocation: '$14k'
				},
				{
					id: 'D3',
					title: 'Evidence delivery and hardening',
					detail:
						'Expose the same committed finding through the API and dashboard, deliver it once through Telegram, and build a deny-by-default Stellar control plane.',
					acceptance:
						'API, UI, and alert evidence agree; missing or invalid origin and admin credentials fail closed in integration tests.',
					allocation: '$10k'
				}
			]
		},
		{
			id: 'history',
			title: 'Historical and operator readiness',
			window: 'Months 3–4',
			allocation: '$33k',
			deliverables: [
				{
					id: 'D4',
					title: 'Historical replay',
					detail:
						'Reconcile a historical Galexie, Captive Core, or equivalent range with the live tail and preserve deterministic fact identity.',
					acceptance:
						'Historical and live paths converge on the same materialized state; replay cannot send live alerts; restore and reprocess tests pass.',
					allocation: '$12k'
				},
				{
					id: 'D5',
					title: 'Control-plane history',
					detail:
						'Version classic account signers and thresholds plus supported contract governance and protocol role attribution.',
					acceptance:
						'Ten protocol families carry time-versioned role attribution with source and confidence; unsupported governance remains unknown.',
					allocation: '$14k'
				},
				{
					id: 'D6',
					title: 'Protocol decoders and pilots',
					detail:
						'Run initial Blend, Reflector, and SDEX sequences and calibrate findings with protocol operators.',
					acceptance:
						'Three pilot teams configure real contracts or accounts and participate in documented false-positive and actionability review.',
					allocation: '$7k'
				}
			]
		},
		{
			id: 'mainnet',
			title: 'Usable Mainnet launch',
			window: 'Months 5–6',
			allocation: '$31k',
			deliverables: [
				{
					id: 'D7',
					title: 'Operations and recoverability',
					detail:
						'Publish coverage and status, test source failover, backup and clean-environment restore, and operate from an incident runbook.',
					acceptance:
						'A public 30-day measurement report includes availability, lag, continuity, failover, and recovery evidence.',
					allocation: '$10k'
				},
				{
					id: 'D8',
					title: 'Public API and onboarding',
					detail:
						'Publish versioned API documentation, a discoverable dashboard, and an assisted or self-service protocol onboarding path.',
					acceptance:
						'Five willing pilot or protocol teams are onboarded and Mainnet accounts, evidence URLs, and coverage are visible to reviewers.',
					allocation: '$12k'
				},
				{
					id: 'D9',
					title: 'Quality and sustainability report',
					detail:
						'Publish measured parse success, corpus recall, alert latency and actionability alongside security limits, costs, revenue, and maintenance ownership.',
					acceptance:
						'Provisional targets are replaced or confirmed from public baselines and the post-grant operating plan names accountable owners.',
					allocation: '$9k'
				}
			]
		}
	];

	const submissionGates = [
		'Obtain written SCF confirmation that Open Track is the correct route.',
		'Publish the reviewer repository, architecture brief, reproducible POC, and short demo.',
		'Complete five interviews, three permissioned quotes, and two pilot or LOI statements.',
		'Name delivery owners and reconcile dates, days, rates, and costs to exactly $100,000.',
		'Publish the control-plane threat model and negative authorization tests.',
		'Define free access, revenue, infrastructure cost, support, and post-grant maintenance ownership.'
	] as const;
</script>

<svelte:head>
	<title>Ferno Pulsar — SCF Build application working draft</title>
	<meta
		name="description"
		content="Unlisted working draft of the Ferno Pulsar Stellar Community Fund Build application."
	/>
	<meta name="robots" content="noindex, nofollow, noarchive" />
	<meta name="googlebot" content="noindex, nofollow, noarchive" />
	<link rel="preconnect" href="https://fonts.googleapis.com" />
	<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
	<link
		href="https://fonts.googleapis.com/css2?family=Archivo+SemiCondensed:wght@500;600&family=Fragment+Mono&family=Source+Sans+3:wght@400;500;600&display=swap"
		rel="stylesheet"
	/>
	<meta name="theme-color" content="#f4f5f2" />
</svelte:head>

<div class="application-page">
	<section class="application-cover" aria-labelledby="application-title">
		<div class="application-width">
			<a class="back-link" href={resolve('/')}>← Product homepage</a>
			<div class="cover-grid">
				<div>
					<h1 id="application-title">Ferno Pulsar<br />SCF Build application</h1>
				</div>
				<div class="cover-copy">
					<p>
						Working draft for a Stellar-specific post-deployment security monitor. This page is
						unlisted and excluded from search indexing; it contains no private applicant data.
					</p>
					<dl>
						<div><dt>Request</dt><dd>$100,000 worth of XLM</dd></div>
						<div><dt>Duration</dt><dd>Six months</dd></div>
						<div><dt>Track</dt><dd>Open — provisional</dd></div>
						<div><dt>Status</dt><dd>Not ready to submit</dd></div>
					</dl>
				</div>
			</div>
		</div>
	</section>

	<section class="application-section" aria-labelledby="narrative-title">
		<div class="application-width">
			<div class="section-heading">
				<span aria-hidden="true">01</span>
				<h2 id="narrative-title">Product and ecosystem value</h2>
			</div>
			<div class="narrative-grid">
				<p class="lead">
					Stellar protocols can be audited before deployment and still change afterward. Wasm
					can be upgraded, administrative roles can rotate, signer thresholds can weaken, and
					an authorized oracle update can still carry an unsafe price.
				</p>
				<div>
					<p>
						Ferno Pulsar builds a replayable history of contract code, control surfaces, and
						protocol activity, then delivers findings through an API, dashboard, and Telegram.
						Each finding retains chain coordinates, raw evidence provenance, parser version,
						applicable Wasm version, and confidence.
					</p>
					<p>
						The product complements audits, formal verification, explorers, and generic data
						pipelines. Its wedge is deterministic post-deployment change-control evidence for
						protocol security and operations teams.
					</p>
				</div>
			</div>
		</div>
	</section>

	<section class="application-section evidence-section" aria-labelledby="evidence-title">
		<div class="application-width">
			<div class="section-heading">
				<span aria-hidden="true">02</span>
				<h2 id="evidence-title">Application evidence</h2>
			</div>
			<p class="section-intro">
				Research coverage is not adoption. Each row must end with a stable reviewer URL,
				named owner, or permissioned external proof before submission.
			</p>
			<ol class="evidence-register">
				{#each evidence as item, index (item.id)}
					<li data-state={item.state}>
						<span class="row-index">{String(index + 1).padStart(2, '0')}</span>
						<div>
							<strong>{item.artifact}</strong>
							<p>{item.detail}</p>
						</div>
						<p class="row-state">{item.state}</p>
					</li>
				{/each}
			</ol>
		</div>
	</section>

	<section class="application-section" aria-labelledby="deliverables-title">
		<div class="application-width">
			<div class="section-heading">
				<span aria-hidden="true">03</span>
				<h2 id="deliverables-title">Three tranches, nine deliverables</h2>
			</div>
			<p class="section-intro">
				The $100k allocation below is provisional. It reconciles arithmetically, but it must
				be rebuilt from named contributors, actual role days, rates, dates, and availability.
			</p>

			<div class="tranche-list">
				{#each tranches as tranche, trancheIndex (tranche.id)}
					<article class="tranche">
						<header>
							<span>{String(trancheIndex + 1).padStart(2, '0')}</span>
							<h3>{tranche.title}</h3>
							<p>{tranche.window}</p>
							<strong>{tranche.allocation}</strong>
						</header>
						<ol>
							{#each tranche.deliverables as deliverable (deliverable.id)}
								<li>
									<span class="deliverable-id">{deliverable.id}</span>
									<div>
										<h4>{deliverable.title}</h4>
										<p>{deliverable.detail}</p>
									</div>
									<p class="acceptance"><strong>Acceptance</strong>{deliverable.acceptance}</p>
									<strong class="allocation">{deliverable.allocation}</strong>
								</li>
							{/each}
						</ol>
					</article>
				{/each}
			</div>
		</div>
	</section>

	<section class="application-section gates-section" aria-labelledby="gates-title">
		<div class="application-width gates-grid">
			<div class="section-heading">
				<span aria-hidden="true">04</span>
				<h2 id="gates-title">Submission gates</h2>
			</div>
			<div>
				<ol class="gate-list">
					{#each submissionGates as gate, index (gate)}
						<li><span>{String(index + 1).padStart(2, '0')}</span><p>{gate}</p></li>
					{/each}
				</ol>
				<div class="official-links">
					<a href="https://communityfund.stellar.org/awards" rel="noreferrer" target="_blank">
						SCF awards and current round ↗
					</a>
					<a
						href="https://stellar.gitbook.io/scf-handbook/scf-awards/build-award/budget-and-deliverable-guidelines"
						rel="noreferrer"
						target="_blank">Budget and deliverable guidelines ↗</a
					>
				</div>
			</div>
		</div>
	</section>
</div>

<style>
	:global(body) {
		background: #f4f5f2;
	}

	.application-page {
		--field: #f4f5f2;
		--paper: #ffffff;
		--ink: #151418;
		--muted: #67636c;
		--rule: #d6d7d0;
		--violet: #6436e8;
		--violet-soft: #eeeaff;
		--anomaly: #d9546b;
		background: var(--field);
		color: var(--ink);
		font-family: 'Source Sans 3', sans-serif;
	}

	.application-width {
		width: min(1320px, calc(100vw - 4rem));
		margin: 0 auto;
	}

	h1,
	h2,
	h3,
	h4,
	p,
	dl,
	dd {
		margin: 0;
	}

	h1,
	h2,
	h3,
	h4 {
		font-family: 'Archivo SemiCondensed', sans-serif;
		font-weight: 600;
	}

	h1 {
		font-size: clamp(3.7rem, 7vw, 6.5rem);
		line-height: 0.9;
		letter-spacing: -0.055em;
	}

	h2 {
		max-width: 16ch;
		font-size: clamp(2.4rem, 4vw, 4.2rem);
		line-height: 0.96;
		letter-spacing: -0.045em;
	}

	p {
		color: var(--muted);
		font-size: 1rem;
		line-height: 1.6;
	}

	a {
		color: inherit;
		text-underline-offset: 0.2em;
	}

	.application-cover {
		padding: 2rem 0 clamp(5rem, 9vw, 8rem);
		border-bottom: 1px solid var(--rule);
	}

	.back-link {
		display: inline-block;
		margin-bottom: clamp(4rem, 8vw, 7rem);
		font-size: 0.8rem;
		font-weight: 600;
	}

	.cover-grid {
		display: grid;
		grid-template-columns: repeat(12, minmax(0, 1fr));
		gap: 1.5rem;
		align-items: end;
	}

	.cover-grid > div:first-child {
		grid-column: 1 / span 7;
	}

	.cover-copy {
		grid-column: 8 / -1;
		display: grid;
		gap: 2rem;
	}

	.cover-copy > p {
		color: #37323d;
		font-size: 1.12rem;
	}

	.cover-copy dl {
		border-top: 1px solid var(--ink);
	}

	.cover-copy dl > div {
		display: grid;
		grid-template-columns: 7rem minmax(0, 1fr);
		gap: 1rem;
		padding: 0.7rem 0;
		border-bottom: 1px solid var(--rule);
	}

	.cover-copy dt {
		color: var(--muted);
		font-size: 0.76rem;
	}

	.cover-copy dd {
		font-size: 0.88rem;
		font-weight: 600;
	}

	.application-section {
		padding: clamp(5rem, 9vw, 8rem) 0;
		border-bottom: 1px solid var(--rule);
	}

	.evidence-section,
	.gates-section {
		background: var(--paper);
	}

	.section-heading {
		display: grid;
		grid-template-columns: 2.5rem minmax(0, 1fr);
		gap: 1rem;
		align-items: start;
	}

	.section-heading > span,
	.row-index,
	.row-state,
	.tranche > header > span,
	.deliverable-id,
	.gate-list span {
		color: var(--violet);
		font-family: 'Fragment Mono', monospace;
		font-size: 0.68rem;
	}

	.section-heading > span {
		padding-top: 0.5rem;
	}

	.narrative-grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: clamp(3rem, 8vw, 8rem);
		margin-top: clamp(4rem, 7vw, 6rem);
		padding-top: 1.2rem;
		border-top: 1px solid var(--ink);
	}

	.narrative-grid .lead {
		color: #2f2a34;
		font-size: 1.35rem;
		font-weight: 500;
		line-height: 1.5;
	}

	.narrative-grid > div {
		display: grid;
		gap: 1.2rem;
	}

	.section-intro {
		max-width: 48rem;
		margin: 2rem 0 0 3.5rem;
		color: #37323d;
	}

	.evidence-register,
	.tranche-list,
	.tranche ol,
	.gate-list {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.evidence-register {
		margin-top: clamp(3rem, 6vw, 5rem);
		border-top: 1px solid var(--ink);
	}

	.evidence-register li {
		display: grid;
		grid-template-columns: 2.5rem minmax(0, 1fr) 9rem;
		gap: 1rem;
		padding: 1rem 0;
		border-bottom: 1px solid var(--rule);
	}

	.evidence-register li strong {
		font-family: 'Archivo SemiCondensed', sans-serif;
		font-size: 1rem;
	}

	.evidence-register li div p {
		max-width: 58rem;
		margin-top: 0.25rem;
		font-size: 0.82rem;
		line-height: 1.45;
	}

	.row-state {
		justify-self: end;
		color: var(--muted);
		text-align: right;
	}

	.evidence-register li[data-state='Ready locally'] .row-state {
		color: var(--violet);
	}

	.evidence-register li[data-state='In progress'] .row-state {
		color: var(--anomaly);
	}

	.tranche-list {
		margin-top: clamp(3rem, 6vw, 5rem);
		border-top: 1px solid var(--ink);
	}

	.tranche > header {
		display: grid;
		grid-template-columns: 2.5rem minmax(0, 1fr) 10rem 6rem;
		gap: 1rem;
		align-items: baseline;
		padding: 1.1rem 0;
		border-bottom: 1px solid var(--ink);
		background: var(--violet-soft);
	}

	.tranche > header > *:first-child {
		padding-left: 0.75rem;
	}

	.tranche > header h3 {
		font-size: 1.35rem;
	}

	.tranche > header p {
		font-size: 0.82rem;
	}

	.tranche > header > strong {
		padding-right: 0.75rem;
		color: var(--violet);
		font-family: 'Archivo SemiCondensed', sans-serif;
		font-size: 1.4rem;
		text-align: right;
	}

	.tranche li {
		display: grid;
		grid-template-columns: 2.5rem minmax(12rem, 1.2fr) minmax(16rem, 1.5fr) 6rem;
		gap: 1rem;
		padding: 1.1rem 0;
		border-bottom: 1px solid var(--rule);
	}

	.deliverable-id {
		padding-left: 0.75rem;
	}

	.tranche h4 {
		font-size: 1rem;
	}

	.tranche li div p,
	.acceptance {
		margin-top: 0.3rem;
		font-size: 0.8rem;
		line-height: 1.45;
	}

	.acceptance {
		margin-top: 0;
	}

	.acceptance strong {
		display: block;
		margin-bottom: 0.25rem;
		color: var(--ink);
		font-size: 0.74rem;
	}

	.allocation {
		padding-right: 0.75rem;
		font-family: 'Fragment Mono', monospace;
		font-size: 0.72rem;
		text-align: right;
	}

	.gates-grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: clamp(3rem, 8vw, 8rem);
	}

	.gate-list {
		border-top: 1px solid var(--ink);
	}

	.gate-list li {
		display: grid;
		grid-template-columns: 2.5rem minmax(0, 1fr);
		gap: 0.8rem;
		padding: 0.8rem 0;
		border-bottom: 1px solid var(--rule);
	}

	.gate-list p {
		font-size: 0.86rem;
		line-height: 1.45;
	}

	.official-links {
		display: flex;
		flex-wrap: wrap;
		gap: 0.8rem 1.2rem;
		margin-top: 2rem;
		font-size: 0.78rem;
		font-weight: 600;
	}

	@media (max-width: 900px) {
		.tranche li {
			grid-template-columns: 2.5rem minmax(0, 1fr) 5rem;
		}

		.acceptance {
			grid-column: 2;
		}

		.allocation {
			grid-column: 3;
			grid-row: 1;
		}
	}

	@media (max-width: 760px) {
		.application-width {
			width: min(100vw - 2rem, 1320px);
		}

		h1 {
			font-size: clamp(3.1rem, 15vw, 4.8rem);
		}

		h2 {
			font-size: clamp(2.3rem, 11vw, 3.5rem);
		}

		.cover-grid,
		.narrative-grid,
		.gates-grid {
			grid-template-columns: 1fr;
		}

		.cover-grid > div:first-child,
		.cover-copy {
			grid-column: 1;
		}

		.cover-copy {
			margin-top: 2rem;
		}

		.section-intro {
			margin-left: 0;
		}

		.evidence-register li {
			grid-template-columns: 2rem minmax(0, 1fr);
		}

		.row-state {
			grid-column: 2;
			justify-self: start;
			text-align: left;
		}

		.tranche > header {
			grid-template-columns: 2rem minmax(0, 1fr) auto;
			gap: 0.5rem 0.8rem;
		}

		.tranche > header h3 {
			grid-column: 2;
		}

		.tranche > header p {
			grid-column: 2;
		}

		.tranche > header > strong {
			grid-column: 3;
			grid-row: 1;
		}

		.tranche li {
			grid-template-columns: 2rem minmax(0, 1fr) auto;
			gap: 0.5rem 0.8rem;
		}

		.acceptance {
			grid-column: 2 / -1;
		}
	}
</style>
