<script lang="ts">
	import StatusNote from '$lib/components/docs/StatusNote.svelte';

	type RoadmapItem = {
		status: 'Implemented' | 'In progress' | 'Planned';
		title: string;
		body: string;
		outcomes: readonly string[];
	};

	const roadmap: readonly RoadmapItem[] = [
		{
			status: 'Implemented',
			title: 'Archive-backed evidence core',
			body: 'The current POC proves that a local Stellar archive can produce exact, queryable dependency and version history with bounded resources.',
			outcomes: [
				'Local Galexie mirror and resumable compact Rust indexer',
				'Exact wallet-to-contract reverse index and source-backed protocol roots',
				'Contract executable history, Wasm observations, and lifecycle events',
				'Wallet activity projection, read-only API, and real-data demo',
				'Bounded YieldBlox historical finding and controller watcher'
			]
		},
		{
			status: 'In progress',
			title: 'Shared archive and live indexing core',
			body: 'Extract the optimized historical logic behind a versioned input and module boundary, then add a restart-safe recent-ledger tail.',
			outcomes: [
				'RPC getLedgers acquisition with explicit oldest and newest source range',
				'One normalized ledger input for archive, RPC, and later Captive Core',
				'Atomic fact, finding, and checkpoint commit',
				'Automatic local-archive gap fill before live resumes',
				'Archive/live parity tests over overlapping ledgers'
			]
		},
		{
			status: 'Planned',
			title: 'Backfillable module framework',
			body: 'Make new protocol parsers and derived indexes deployable without losing earlier history or blocking the active read generation.',
			outcomes: [
				'Versioned module and output-schema contract',
				'Offline replay into isolated staging generations',
				'Determinism, idempotency, coverage, and resource promotion gates',
				'Live catch-up buffer and atomic generation switch',
				'Historical findings retained with notification delivery disabled'
			]
		},
		{
			status: 'Planned',
			title: 'Broader protocol attribution',
			body: 'Move from verified roots toward complete, time-aware protocol surfaces while preserving every unknown contract as an address-backed record.',
			outcomes: [
				'Soroswap pair and DeFindex vault factory-child discovery',
				'Aquarius and Phoenix pool expansion',
				'Blend deployment and Reflector runtime-root coverage',
				'Time-valid roles, controller history, and attribution conflicts',
				'Source-backed Templar, Sushi, and additional Stellar ecosystem roots'
			]
		},
		{
			status: 'Planned',
			title: 'Protocol-specific security intelligence',
			body: 'Add semantic decoders where generic contract facts cannot explain risk or intent.',
			outcomes: [
				'Blend, Reflector, and Classic SDEX decoders for generalized YieldBlox-style detection',
				'Aquarius upgrade-delay and emergency-control monitoring',
				'DeFindex vault roles, Phoenix controls, FxDAO roles, and Soroswap adapters',
				'Unknown-Wasm coverage findings instead of stale decoding',
				'Deterministic evidence separated from AI-assisted impact explanation'
			]
		},
		{
			status: 'Planned',
			title: 'Wider monitoring surface',
			body: 'Correlate on-chain changes with the technical places developers otherwise monitor manually.',
			outcomes: [
				'Soroban Governor proposals and execution payloads',
				'Admin, guardian, treasury, oracle, signer, and threshold histories',
				'GitHub commits, releases, deployment manifests, and security advisories',
				'Stellar SDK, package registry, protocol, Core, and RPC releases',
				'Project-specific pending multisig feeds where a verifiable source exists'
			]
		},
		{
			status: 'Planned',
			title: 'Reliable delivery and team workflows',
			body: 'Turn committed findings into an operational alert product only after detection and recovery guarantees are observable.',
			outcomes: [
				'Durable idempotent alert outbox and Telegram delivery',
				'Webhooks, email, per-user history, and delivery receipts',
				'Application dependency subscriptions and impact routing',
				'Team-reviewed enterprise alerts with evidence and audit trail',
				'Paid entitlements and Stellar USDC settlement after delivery is proven'
			]
		}
	] as const;
</script>

<svelte:head>
	<title>Technical roadmap · Stargazer documentation</title>
	<meta
		name="description"
		content="Stargazer's technical roadmap for live Stellar ingestion, backfillable indexers, broader protocol coverage, more signal sources, and reliable alert delivery."
	/>
	<link rel="canonical" href="https://stargazer.ferno.ag/docs/roadmap" />
</svelte:head>

<article class="doc-page">
	<header class="doc-hero">
		<p class="doc-kicker">Reference / Roadmap</p>
		<h1>Expand coverage without weakening evidence.</h1>
		<p class="doc-lede">
			The roadmap has two parallel goals: add more Stellar protocols and expand the monitoring surface
			around each dependency. The local archive and shared indexer core come first because every later
			decoder, signal source, and alert needs a reproducible history and a safe live handoff.
		</p>
	</header>

	<StatusNote status="In progress" title="Roadmap convention">
		Items are ordered by dependency, not by marketing priority or fixed calendar date. “Implemented”
		means code and evidence exist in this repository; “Planned” remains a target until its acceptance
		conditions are demonstrable.
	</StatusNote>

	<section class="doc-section" id="priorities">
		<div class="doc-section-heading">
			<p class="doc-kicker">01 / Delivery sequence</p>
			<h2>Technical milestones.</h2>
		</div>
		<ol class="roadmap-list">
			{#each roadmap as item, index (item.title)}
				<li>
					<header>
						<span>{String(index + 1).padStart(2, '0')}</span>
						<div>
							<p data-status={item.status.toLowerCase().replace(' ', '-')}>{item.status}</p>
							<h3>{item.title}</h3>
						</div>
					</header>
					<p>{item.body}</p>
					<ul>
						{#each item.outcomes as outcome (outcome)}
							<li>{outcome}</li>
						{/each}
					</ul>
				</li>
			{/each}
		</ol>
	</section>

	<section class="doc-section" id="protocol-order">
		<div class="doc-section-heading">
			<p class="doc-kicker">02 / Protocol expansion</p>
			<h2>Prioritize by dependency risk and verifiable surface.</h2>
		</div>
		<div class="table-scroll">
			<table>
				<thead>
					<tr>
						<th>Wave</th>
						<th>Coverage</th>
						<th>Reason</th>
					</tr>
				</thead>
				<tbody>
					<tr>
						<td>First</td>
						<td>Blend + Reflector + Classic SDEX</td>
						<td>Cross-protocol lending and oracle dependency with a confirmed incident fixture.</td>
					</tr>
					<tr>
						<td>Next</td>
						<td>Soroswap + DeFindex</td>
						<td>Factory events make child discovery testable and unlock broader wallet attribution.</td>
					</tr>
					<tr>
						<td>Then</td>
						<td>Aquarius + Phoenix + FxDAO</td>
						<td>Known administration and runtime surfaces support high-signal control monitoring.</td>
					</tr>
					<tr>
						<td>Close gaps</td>
						<td>Templar, Sushi, and new ecosystem projects</td>
						<td>Attribute production roots before claiming ecosystem-wide DeFi coverage.</td>
					</tr>
				</tbody>
			</table>
		</div>
	</section>

	<section class="doc-section" id="signals">
		<div class="doc-section-heading">
			<p class="doc-kicker">03 / Signal expansion</p>
			<h2>Monitor the dependency, not only its contract address.</h2>
		</div>
		<div class="signal-matrix">
			<section>
				<span>On-chain runtime</span>
				<h3>What executes</h3>
				<p>Deployments, executable changes, calls, events, state, restorations, failures, and code hashes.</p>
			</section>
			<section>
				<span>Control plane</span>
				<h3>Who can change it</h3>
				<p>Admins, guardians, treasuries, governors, signers, thresholds, controllers, and executed payloads.</p>
			</section>
			<section>
				<span>Developer surface</span>
				<h3>What applications consume</h3>
				<p>Repositories, releases, SDKs, packages, deployment manifests, interfaces, and breaking changes.</p>
			</section>
			<section>
				<span>Ecosystem context</span>
				<h3>What changes the risk</h3>
				<p>Governance proposals, security advisories, Stellar protocol releases, Core/RPC changes, and incidents.</p>
			</section>
		</div>
	</section>

	<section class="doc-section" id="acceptance">
		<div class="doc-section-heading">
			<p class="doc-kicker">04 / Acceptance standard</p>
			<h2>Coverage claims need proof.</h2>
		</div>
		<ul class="doc-check-list">
			<li>every monitored protocol publishes its known contracts, roles, source URLs, and observed-at boundary;</li>
			<li>factory-backed protocols report discovered children and unexplained conflicts;</li>
			<li>every versioned decoder lists recognized Wasm hashes and emits unknown-code coverage findings;</li>
			<li>every live module has passed deterministic archive replay and source-parity tests;</li>
			<li>every alert retains coordinates, primary evidence, module version, confidence, and coverage limits;</li>
			<li>every delivery channel exposes retry and receipt state independently of detection;</li>
			<li>status pages distinguish delayed, unavailable, partial, and healthy sources.</li>
		</ul>
	</section>
</article>
