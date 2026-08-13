<script lang="ts">
	import { resolve } from '$app/paths';

	type Signal = {
		index: string;
		title: string;
		body: string;
	};

	const signals: readonly Signal[] = [
		{
			index: '01',
			title: 'Protocol changes',
			body: 'Soroban contract upgrades, new deployments, governance actions, and control changes.'
		},
		{
			index: '02',
			title: 'Developer dependencies',
			body: 'Repository activity, SDK releases, packages, and infrastructure your application builds on.'
		},
		{
			index: '03',
			title: 'Application impact',
			body: 'A versioned map connects each meaningful event to the protocols, contracts, and wallets you use.'
		},
		{
			index: '04',
			title: 'Evidence-backed alerts',
			body: 'Concise, AI-assisted analysis explains what changed, why it matters, and where to verify it.'
		}
	];

	const workflow = [
		{
			index: '01',
			verb: 'Observe',
			text: 'Trusted ledger, governance, repository, SDK, package, and release sources.'
		},
		{
			index: '02',
			verb: 'Correlate',
			text: 'Events joined to a versioned protocol and application dependency map.'
		},
		{
			index: '03',
			verb: 'Assess',
			text: 'Deterministic rules and evidence-grounded AI identify material changes.'
		},
		{
			index: '04',
			verb: 'Deliver',
			text: 'Actionable intelligence through the dashboard, API, and notification channels.'
		}
	] as const;

	const demoProof = [
		'Wallet-to-contract relationships reconstructed from indexed Stellar data',
		'Known protocols attributed without hiding unknown on-chain contracts',
		'Exact executable changes with ledger and transaction evidence',
		'Historical incidents connected to exposed wallets and protocol surfaces'
	] as const;

	const grantScope = [
		'Continuous Soroban deployment, upgrade, and protocol-control monitoring',
		'Repository, governance, SDK, package, and release signals',
		'AI-assisted dependency impact analysis and concise summaries',
		'Subscribable real-time delivery through Telegram, API, and dashboard'
	] as const;

	const roadmap = [
		{
			phase: '01',
			title: 'Map the ecosystem',
			body: 'Onboard key Stellar protocols, Soroban contracts, developer tooling, controllers, and trusted sources.'
		},
		{
			phase: '02',
			title: 'Monitor continuously',
			body: 'Tail on-chain changes and connect them with governance, repositories, SDKs, packages, and releases.'
		},
		{
			phase: '03',
			title: 'Explain the impact',
			body: 'Correlate changes with application dependencies and produce concise, evidence-grounded analysis.'
		},
		{
			phase: '04',
			title: 'Deliver the signal',
			body: 'Let teams subscribe to the dependencies they use and receive timely dashboard, API, and Telegram alerts.'
		}
	] as const;

	const yieldBloxWallet = 'GBO7VUL2TOKPWFAWKATIW7K3QYA7WQ63VDY5CAE6AFUUX6BHZBOC2WXC';
</script>

<svelte:head>
	<title>Stargazer · Live protocol intelligence for Stellar developers</title>
	<meta
		name="description"
		content="Stargazer connects Stellar and Soroban protocol changes to the applications that depend on them, then delivers concise, evidence-backed intelligence."
	/>
	<meta property="og:title" content="Stargazer · Protocol intelligence for Stellar" />
	<meta
		property="og:description"
		content="See what changed across Stellar protocols and know what it affects."
	/>
</svelte:head>

<section class="hero" aria-labelledby="hero-title">
	<div class="hero-copy">
		<p class="kicker"><span aria-hidden="true"></span> Protocol intelligence for Stellar</p>
		<h1 id="hero-title">See what changed.<br /><em>Know what it affects.</em></h1>
		<p class="hero-lede">
			Stargazer is building the live intelligence layer for Stellar developers. It follows the
			contracts, protocols, governance, repositories, SDKs, and releases behind an application—and
			turns meaningful changes into concise, actionable alerts.
		</p>

		<div class="hero-actions">
			<a class="action action-primary" href={resolve('/app/alerts')}>
				Explore the Stellar demo <span aria-hidden="true">→</span>
			</a>
			<a class="action action-secondary" href={resolve('/app/protocols')}>
				See monitored protocols
			</a>
		</div>

		<div class="availability" aria-label="Current product status">
			<div>
				<strong>Available today</strong>
				<span>Archive-backed Stellar evidence and real indexed events</span>
			</div>
			<div>
				<strong>In development</strong>
				<span>Continuous multi-source monitoring and real-time delivery</span>
			</div>
		</div>
	</div>

	<figure class="hero-visual" aria-label="Stargazer monitoring Stellar protocol signals">
		<img src="/brand/stargazer-cosmic-hero.jpeg" alt="" />
		<div class="visual-shade"></div>
		<figcaption>
			<span>Stellar Mainnet</span>
			<strong>Protocol signal acquired</strong>
			<small>Contract · governance · dependency</small>
		</figcaption>
	</figure>
</section>

<section class="problem-section" id="problem" aria-labelledby="problem-title">
	<div class="section-index">01 / Problem</div>
	<div class="problem-copy">
		<h2 id="problem-title">Your application can change even when your code does not.</h2>
		<p>
			A contract upgrades. An SDK introduces a breaking change. Governance moves control. A new
			deployment replaces infrastructure your product relies on. The signals are scattered across
			ledgers, repositories, release feeds, and community channels—so teams often discover them only
			after something breaks.
		</p>
	</div>
	<aside>
		<span>The operational gap</span>
		<strong>Critical dependencies evolve continuously. Monitoring them is still manual.</strong>
	</aside>
</section>

<section class="signals-section" id="product" aria-labelledby="signals-title">
	<header class="section-heading">
		<div class="section-index">02 / Product</div>
		<div>
			<p class="eyebrow">One operational view</p>
			<h2 id="signals-title">Follow the infrastructure behind your application.</h2>
		</div>
		<p>
			Stargazer connects on-chain and off-chain evidence so developers can detect change, understand
			the affected surface, and respond from primary sources.
		</p>
	</header>

	<div class="signal-grid">
		{#each signals as signal (signal.index)}
			<article>
				<span>{signal.index}</span>
				<h3>{signal.title}</h3>
				<p>{signal.body}</p>
			</article>
		{/each}
	</div>
</section>

<section class="workflow-section" id="signals" aria-labelledby="workflow-title">
	<div class="workflow-intro">
		<div class="section-index">03 / Signal pipeline</div>
		<p class="eyebrow">From change to action</p>
		<h2 id="workflow-title">Less noise.<br />More consequence.</h2>
		<p>
			The objective is not another activity feed. Stargazer connects a technical event to the systems
			that depend on it, then preserves the evidence behind every conclusion.
		</p>
	</div>

	<ol class="workflow-list">
		{#each workflow as step (step.index)}
			<li>
				<span>{step.index}</span>
				<strong>{step.verb}</strong>
				<p>{step.text}</p>
			</li>
		{/each}
	</ol>
</section>

<section class="demo-section" id="evidence" aria-labelledby="demo-title">
	<div class="demo-copy">
		<div class="section-index">04 / Working demo</div>
		<p class="eyebrow">Real Stellar archive data</p>
		<h2 id="demo-title">See what Stargazer can prove today.</h2>
		<p>
			The current demo reconstructs protocol relationships and technical changes from indexed Stellar
			data. It is an evidence-backed product preview—not yet a production notification service.
		</p>
		<ul>
			{#each demoProof as item (item)}
				<li>{item}</li>
			{/each}
		</ul>
	</div>

	<div class="demo-console">
		<header>
			<span>STARGAZER / STELLAR DEMO</span>
			<i>ARCHIVE CONNECTED</i>
		</header>
		<div class="console-event">
			<div class="event-meta">
				<strong>CRITICAL · RETROSPECTIVE</strong>
				<time datetime="2026-02-22">22 FEB 2026</time>
			</div>
			<p class="event-protocol">YieldBlox / Blend v2</p>
			<h3>Oracle manipulation affected a protocol dependency.</h3>
			<p>
				Stargazer connects the verified incident surface to wallets that interacted with it, preserving
				the contract and transaction evidence.
			</p>
			<a href={resolve('/app/wallet/[address]', { address: yieldBloxWallet })}>
				Check wallet exposure <span aria-hidden="true">→</span>
			</a>
		</div>
		<nav aria-label="Explore the product demo">
			<a href={resolve('/app/alerts')}>Browse alerts <span>→</span></a>
			<a href={resolve('/app/protocols')}>Inspect protocol versions <span>→</span></a>
			<a href={resolve('/app')}>Check another wallet <span>→</span></a>
		</nav>
	</div>
</section>

<section class="firewatch-section" id="firewatch" aria-labelledby="firewatch-title">
	<div class="firewatch-copy">
		<div class="section-index">05 / Proven infrastructure</div>
		<h2 id="firewatch-title">Proven infrastructure, adapted for Stellar.</h2>
		<p>
			Stargazer extends Firewatch, Ferno’s live protocol-intelligence platform for Solana. The
			foundation is already used by developers, teams, and ecosystem contributors.
		</p>
	</div>
	<div class="firewatch-evidence">
		<div class="platform-lineage" aria-label="Firewatch on Solana extended into Stargazer on Stellar">
			<div>
				<span>Firewatch</span>
				<strong>Solana</strong>
				<em>Live in production</em>
			</div>
			<i aria-hidden="true">→</i>
			<div>
				<span>Stargazer</span>
				<strong>Stellar</strong>
				<em>Integration in development</em>
			</div>
		</div>
		<p class="firewatch-description">
			Firewatch tracks program upgrades, deployments, IDL changes, governance, repositories, and other
			technical events that affect downstream applications. Stargazer adapts that operating model to
			Soroban contracts and the wider Stellar developer stack.
		</p>
		<dl class="firewatch-facts">
			<div><dt>Operational</dt><dd>Live protocol intelligence on Solana</dd></div>
			<div><dt>Used in practice</dt><dd>Adopted by developers and ecosystem teams</dd></div>
			<div><dt>Shared foundation</dt><dd>Monitoring, analysis, and notification infrastructure</dd></div>
		</dl>
	</div>
</section>

<section class="scope-section" aria-labelledby="scope-title">
	<header class="section-heading">
		<div class="section-index">06 / Product status</div>
		<div>
			<p class="eyebrow">The next layer</p>
			<h2 id="scope-title">From archive evidence to continuous intelligence.</h2>
		</div>
		<p>
			Stargazer is onboarding key Stellar protocols and developer tooling, then making their
			technical evolution subscribable.
		</p>
	</header>

	<div class="scope-columns">
		<div>
			<h3>Working in the demo</h3>
			<ul>
				{#each demoProof as item (item)}<li>{item}</li>{/each}
			</ul>
		</div>
		<div>
			<h3>In development</h3>
			<ul>
				{#each grantScope as item (item)}<li>{item}</li>{/each}
			</ul>
		</div>
	</div>
</section>

<section class="roadmap-section" id="roadmap" aria-labelledby="roadmap-title">
	<header>
		<div class="section-index">07 / Roadmap</div>
		<p class="eyebrow">Stellar integration</p>
		<h2 id="roadmap-title">A monitoring layer developers can rely on.</h2>
	</header>
	<ol>
		{#each roadmap as item (item.phase)}
			<li>
				<span>{item.phase}</span>
				<h3>{item.title}</h3>
				<p>{item.body}</p>
			</li>
		{/each}
	</ol>
</section>

<section class="team-section" aria-labelledby="team-title">
	<div>
		<div class="section-index">For protocol teams</div>
		<h2 id="team-title">Need reviewed intelligence, not another unattended feed?</h2>
		<p>
			Ferno can define your dependency baseline, escalation channel, and analyst-review scope. Managed
			monitoring pilots start at <strong>$10,000/year</strong>.
		</p>
	</div>
	<a href="https://x.com/ferno_ag" rel="noreferrer" target="_blank">Discuss team coverage ↗</a>
</section>

<section class="closing-section" aria-labelledby="closing-title">
	<p class="eyebrow">Start with the evidence</p>
	<h2 id="closing-title">Explore real indexed events.<br />Then tell us what your application depends on.</h2>
	<div>
		<a class="action action-primary" href={resolve('/app/alerts')}>
			Open the Stellar demo <span aria-hidden="true">→</span>
		</a>
		<a class="action action-secondary" href="https://x.com/ferno_ag" rel="noreferrer" target="_blank">
			Join the Stellar pilot ↗
		</a>
	</div>
</section>

<style>
	:global(.watch-content) {
		overflow: hidden;
	}

	:global(.watch-content h1),
	:global(.watch-content h2),
	:global(.watch-content h3),
	:global(.watch-content p) {
		text-wrap: pretty;
	}

	.hero,
	.problem-section,
	.signals-section,
	.workflow-section,
	.demo-section,
	.firewatch-section,
	.scope-section,
	.roadmap-section,
	.team-section,
	.closing-section {
		width: min(1320px, calc(100vw - 4rem));
		margin: 0 auto;
	}

	.hero {
		display: grid;
		grid-template-columns: minmax(0, 1.05fr) minmax(25rem, 0.95fr);
		min-height: calc(100svh - 4.25rem);
		align-items: center;
		gap: clamp(2rem, 5vw, 6rem);
		padding: clamp(4rem, 8vw, 7.5rem) 0;
	}

	.kicker,
	.eyebrow,
	.section-index {
		font: 500 0.69rem/1.4 'Fragment Mono', monospace;
		letter-spacing: 0.1em;
		text-transform: uppercase;
	}

	.kicker {
		display: flex;
		align-items: center;
		gap: 0.7rem;
		margin: 0 0 1.5rem;
		color: var(--watch-blue-soft);
	}

	.kicker span {
		width: 1.8rem;
		height: 1px;
		background: var(--watch-blue);
		box-shadow: 0 0 0.8rem var(--watch-blue);
	}

	.hero h1 {
		max-width: 13ch;
		margin: 0;
		font: 500 clamp(3.9rem, 7vw, 7.4rem)/0.9 'Archivo SemiCondensed', sans-serif;
		letter-spacing: -0.06em;
	}

	.hero h1 em {
		color: var(--watch-blue-soft);
		font-style: normal;
	}

	.hero-lede {
		max-width: 43rem;
		margin: 2rem 0 0;
		color: var(--watch-muted);
		font-size: clamp(1rem, 1.45vw, 1.18rem);
		line-height: 1.65;
	}

	.hero-actions,
	.closing-section > div {
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem;
		margin-top: 2rem;
	}

	.action {
		display: inline-flex;
		min-height: 3.15rem;
		align-items: center;
		justify-content: center;
		gap: 1.4rem;
		padding: 0 1.2rem;
		border: 1px solid var(--watch-rule-strong);
		font-size: 0.88rem;
		font-weight: 600;
		text-decoration: none;
	}

	.action-primary {
		border-color: var(--watch-blue);
		background: var(--watch-blue);
		color: #fff;
	}

	.action-primary:hover {
		background: #7890ff;
	}

	.action-secondary:hover {
		border-color: var(--watch-blue-soft);
		color: var(--watch-blue-soft);
	}

	.availability {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		max-width: 48rem;
		margin-top: clamp(2.5rem, 5vw, 4.5rem);
		border-top: 1px solid var(--watch-rule);
		border-bottom: 1px solid var(--watch-rule);
	}

	.availability div {
		padding: 1rem 1rem 1rem 0;
	}

	.availability div + div {
		padding-left: 1rem;
		border-left: 1px solid var(--watch-rule);
	}

	.availability strong,
	.availability span {
		display: block;
	}

	.availability strong {
		color: var(--watch-blue-soft);
		font-size: 0.78rem;
	}

	.availability span {
		margin-top: 0.3rem;
		color: var(--watch-muted);
		font-size: 0.78rem;
		line-height: 1.4;
	}

	.hero-visual {
		position: relative;
		min-height: min(68svh, 47rem);
		margin: 0;
		overflow: hidden;
		border: 1px solid var(--watch-rule);
		background: #02040b;
	}

	.hero-visual::before {
		position: absolute;
		z-index: 2;
		top: 1.5rem;
		left: 1.5rem;
		width: 0.55rem;
		height: 0.55rem;
		border: 1px solid var(--watch-blue-soft);
		border-radius: 50%;
		box-shadow: 0 0 1rem var(--watch-blue);
		content: '';
	}

	.hero-visual img {
		position: absolute;
		width: 100%;
		height: 100%;
		object-fit: cover;
		object-position: right center;
		transform: scale(1.3);
		transform-origin: right center;
	}

	.visual-shade {
		position: absolute;
		inset: 0;
		background: linear-gradient(
			90deg,
			#02040b 0%,
			#02040b 28%,
			rgba(2, 4, 11, 0.88) 43%,
			rgba(2, 4, 11, 0.04) 68%
		);
	}

	.hero-visual figcaption {
		position: absolute;
		z-index: 3;
		right: 1.25rem;
		bottom: 1.25rem;
		left: 1.25rem;
		padding: 1rem;
		border-top: 1px solid rgba(190, 204, 255, 0.45);
		background: rgba(2, 5, 14, 0.82);
		backdrop-filter: blur(12px);
	}

	.hero-visual figcaption span,
	.hero-visual figcaption strong,
	.hero-visual figcaption small {
		display: block;
	}

	.hero-visual figcaption span,
	.hero-visual figcaption small {
		font: 0.64rem/1.4 'Fragment Mono', monospace;
		letter-spacing: 0.08em;
		text-transform: uppercase;
	}

	.hero-visual figcaption span {
		color: var(--watch-blue-soft);
	}

	.hero-visual figcaption strong {
		margin-top: 0.35rem;
		font: 500 1.2rem/1.25 'Archivo SemiCondensed', sans-serif;
	}

	.hero-visual figcaption small {
		margin-top: 0.35rem;
		color: var(--watch-muted);
	}

	.problem-section,
	.workflow-section,
	.demo-section,
	.firewatch-section,
	.team-section {
		border-top: 1px solid var(--watch-rule);
	}

	.problem-section {
		display: grid;
		grid-template-columns: 0.38fr 1.2fr 0.62fr;
		gap: clamp(2rem, 5vw, 5rem);
		padding: clamp(4rem, 8vw, 7rem) 0;
	}

	.section-index {
		color: var(--watch-blue-soft);
	}

	.problem-copy h2,
	.section-heading h2,
	.workflow-intro h2,
	.demo-copy h2,
	.firewatch-copy h2,
	.roadmap-section h2,
	.team-section h2,
	.closing-section h2 {
		margin: 0;
		font: 500 clamp(2.5rem, 5vw, 5.2rem)/0.98 'Archivo SemiCondensed', sans-serif;
		letter-spacing: -0.045em;
	}

	.problem-copy > p,
	.section-heading > p,
	.workflow-intro > p:last-child,
	.demo-copy > p,
	.firewatch-copy > p,
	.team-section p {
		color: var(--watch-muted);
		font-size: 1rem;
		line-height: 1.65;
	}

	.problem-section aside {
		align-self: end;
		padding: 1.2rem 0;
		border-top: 2px solid var(--watch-blue);
		border-bottom: 1px solid var(--watch-rule);
	}

	.problem-section aside span,
	.problem-section aside strong {
		display: block;
	}

	.problem-section aside span {
		color: var(--watch-muted);
		font: 0.64rem/1.4 'Fragment Mono', monospace;
		letter-spacing: 0.08em;
		text-transform: uppercase;
	}

	.problem-section aside strong {
		margin-top: 0.65rem;
		font: 500 1.1rem/1.35 'Archivo SemiCondensed', sans-serif;
	}

	.signals-section,
	.scope-section,
	.roadmap-section {
		padding: clamp(4rem, 8vw, 7rem) 0;
	}

	.section-heading {
		display: grid;
		grid-template-columns: 0.38fr 1.1fr 0.72fr;
		gap: clamp(2rem, 5vw, 5rem);
		align-items: end;
	}

	.eyebrow {
		margin: 0 0 0.9rem;
		color: var(--watch-blue-soft);
	}

	.section-heading h2 {
		font-size: clamp(2.5rem, 4.5vw, 4.7rem);
	}

	.signal-grid {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		margin-top: clamp(3rem, 6vw, 5rem);
		border-top: 1px solid var(--watch-rule-strong);
		border-bottom: 1px solid var(--watch-rule-strong);
	}

	.signal-grid article {
		min-height: 17rem;
		padding: 1.3rem;
		border-right: 1px solid var(--watch-rule);
	}

	.signal-grid article:last-child {
		border-right: 0;
	}

	.signal-grid article > span,
	.workflow-list li > span,
	.roadmap-section li > span {
		color: var(--watch-blue-soft);
		font: 0.68rem/1.4 'Fragment Mono', monospace;
	}

	.signal-grid h3 {
		margin: 5rem 0 0;
		font: 500 1.5rem/1.15 'Archivo SemiCondensed', sans-serif;
	}

	.signal-grid p,
	.workflow-list p,
	.roadmap-section li p {
		color: var(--watch-muted);
		line-height: 1.55;
	}

	.workflow-section {
		display: grid;
		grid-template-columns: minmax(19rem, 0.8fr) minmax(30rem, 1.2fr);
		gap: clamp(3rem, 8vw, 8rem);
		padding: clamp(4rem, 8vw, 7rem) 0;
	}

	.workflow-intro h2 {
		font-size: clamp(3rem, 5.5vw, 5.8rem);
	}

	.workflow-list {
		margin: 0;
		padding: 0;
		border-top: 1px solid var(--watch-rule-strong);
		list-style: none;
	}

	.workflow-list li {
		display: grid;
		grid-template-columns: 3rem 8rem 1fr;
		gap: 1rem;
		align-items: baseline;
		padding: 1.4rem 0;
		border-bottom: 1px solid var(--watch-rule);
	}

	.workflow-list strong {
		font: 500 1.15rem/1.3 'Archivo SemiCondensed', sans-serif;
	}

	.workflow-list p {
		margin: 0;
	}

	.demo-section {
		display: grid;
		grid-template-columns: minmax(20rem, 0.72fr) minmax(30rem, 1.28fr);
		gap: clamp(3rem, 8vw, 8rem);
		padding: clamp(4rem, 8vw, 7rem) 0;
	}

	.demo-copy ul,
	.scope-columns ul {
		margin: 2rem 0 0;
		padding: 0;
		border-top: 1px solid var(--watch-rule);
		list-style: none;
	}

	.demo-copy li,
	.scope-columns li {
		position: relative;
		padding: 0.85rem 0 0.85rem 1.5rem;
		border-bottom: 1px solid var(--watch-rule);
		color: var(--watch-muted);
		line-height: 1.45;
	}

	.demo-copy li::before,
	.scope-columns li::before {
		position: absolute;
		left: 0;
		color: var(--watch-blue-soft);
		content: '✦';
	}

	.demo-console {
		align-self: start;
		border: 1px solid var(--watch-rule-strong);
		background: var(--watch-surface);
	}

	.demo-console > header {
		display: flex;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.85rem 1rem;
		border-bottom: 1px solid var(--watch-rule);
		font: 0.64rem/1.4 'Fragment Mono', monospace;
		letter-spacing: 0.07em;
	}

	.demo-console > header i {
		color: #7ee0b6;
		font-style: normal;
	}

	.console-event {
		padding: clamp(1.5rem, 4vw, 3rem);
	}

	.event-meta {
		display: flex;
		justify-content: space-between;
		gap: 1rem;
		color: var(--watch-muted);
		font: 0.68rem/1.4 'Fragment Mono', monospace;
	}

	.event-meta strong {
		color: #ff7f9d;
	}

	.event-protocol {
		margin: 3.5rem 0 0;
		color: var(--watch-blue-soft);
		font: 0.7rem/1.4 'Fragment Mono', monospace;
		text-transform: uppercase;
	}

	.console-event h3 {
		max-width: 15ch;
		margin: 0.65rem 0 0;
		font: 500 clamp(2rem, 4vw, 3.7rem)/1 'Archivo SemiCondensed', sans-serif;
		letter-spacing: -0.035em;
	}

	.console-event > p:last-of-type {
		max-width: 38rem;
		color: var(--watch-muted);
		line-height: 1.55;
	}

	.console-event > a,
	.demo-console nav a {
		font-size: 0.87rem;
		font-weight: 600;
		text-decoration: none;
	}

	.console-event > a {
		display: inline-flex;
		gap: 1rem;
		margin-top: 1rem;
		color: var(--watch-blue-soft);
	}

	.demo-console nav {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		border-top: 1px solid var(--watch-rule);
	}

	.demo-console nav a {
		display: flex;
		justify-content: space-between;
		gap: 1rem;
		padding: 1rem;
		border-right: 1px solid var(--watch-rule);
	}

	.demo-console nav a:last-child {
		border-right: 0;
	}

	.demo-console nav a:hover {
		background: rgba(109, 131, 255, 0.08);
		color: var(--watch-blue-soft);
	}

	.firewatch-section {
		display: grid;
		grid-template-columns: minmax(23rem, 0.95fr) minmax(30rem, 1.05fr);
		gap: clamp(3rem, 6vw, 6rem);
		align-items: start;
		padding: clamp(4rem, 8vw, 7rem) 0;
	}

	.firewatch-copy h2 {
		max-width: 15ch;
		margin-top: 0.9rem;
		font-size: clamp(2.8rem, 3.8vw, 4rem);
	}

	.firewatch-copy > p {
		max-width: 35rem;
		margin: 1.6rem 0 0;
	}

	.firewatch-evidence {
		border-top: 1px solid var(--watch-rule-strong);
	}

	.platform-lineage {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
		gap: 1.5rem;
		align-items: center;
		padding: 1.4rem 0;
		border-bottom: 1px solid var(--watch-rule);
	}

	.platform-lineage > div:last-child {
		text-align: right;
	}

	.platform-lineage span,
	.platform-lineage strong,
	.platform-lineage em {
		display: block;
	}

	.platform-lineage span {
		color: var(--watch-blue-soft);
		font: 500 0.67rem/1.4 'Fragment Mono', monospace;
		letter-spacing: 0.08em;
		text-transform: uppercase;
	}

	.platform-lineage strong {
		margin-top: 0.3rem;
		font: 500 1.65rem/1.15 'Archivo SemiCondensed', sans-serif;
	}

	.platform-lineage em {
		margin-top: 0.3rem;
		color: var(--watch-muted);
		font-size: 0.78rem;
		font-style: normal;
	}

	.platform-lineage > i {
		color: var(--watch-blue-soft);
		font: normal 1rem/1 'Fragment Mono', monospace;
	}

	.firewatch-description {
		max-width: 50rem;
		margin: 1.6rem 0;
		color: var(--watch-muted);
		font-size: 1rem;
		line-height: 1.65;
	}

	.firewatch-facts {
		margin: 0;
		border-top: 1px solid var(--watch-rule);
	}

	.firewatch-facts div {
		display: grid;
		grid-template-columns: minmax(8.5rem, 0.4fr) minmax(0, 1fr);
		gap: 1.5rem;
		align-items: baseline;
		padding: 1rem 0;
		border-bottom: 1px solid var(--watch-rule);
	}

	.firewatch-facts dt {
		font: 500 0.95rem/1.35 'Archivo SemiCondensed', sans-serif;
	}

	.firewatch-facts dd {
		margin: 0;
		color: var(--watch-muted);
		font-size: 0.88rem;
		line-height: 1.45;
	}

	.scope-columns {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: 1px;
		margin-top: clamp(3rem, 6vw, 5rem);
		border: 1px solid var(--watch-rule-strong);
		background: var(--watch-rule);
	}

	.scope-columns > div {
		padding: clamp(1.5rem, 4vw, 3rem);
		background: var(--watch-surface);
	}

	.scope-columns h3 {
		margin: 0;
		font: 500 1.5rem/1.2 'Archivo SemiCondensed', sans-serif;
	}

	.roadmap-section {
		display: grid;
		grid-template-columns: minmax(18rem, 0.65fr) minmax(32rem, 1.35fr);
		gap: clamp(3rem, 8vw, 8rem);
		border-top: 1px solid var(--watch-rule);
	}

	.roadmap-section h2 {
		font-size: clamp(2.5rem, 4.5vw, 4.7rem);
	}

	.roadmap-section ol {
		margin: 0;
		padding: 0;
		border-top: 1px solid var(--watch-rule-strong);
		list-style: none;
	}

	.roadmap-section li {
		display: grid;
		grid-template-columns: 3rem 12rem 1fr;
		gap: 1rem;
		align-items: baseline;
		padding: 1.4rem 0;
		border-bottom: 1px solid var(--watch-rule);
	}

	.roadmap-section li h3 {
		margin: 0;
		font: 500 1.15rem/1.3 'Archivo SemiCondensed', sans-serif;
	}

	.roadmap-section li p {
		margin: 0;
	}

	.team-section {
		display: flex;
		justify-content: space-between;
		gap: 3rem;
		align-items: end;
		padding: 3rem 0;
	}

	.team-section > div {
		max-width: 58rem;
	}

	.team-section h2 {
		margin-top: 0.7rem;
		font-size: clamp(2rem, 4vw, 3.8rem);
	}

	.team-section a {
		flex: 0 0 auto;
		padding-bottom: 0.35rem;
		border-bottom: 1px solid var(--watch-blue-soft);
		color: var(--watch-blue-soft);
		font-weight: 600;
		text-decoration: none;
	}

	.closing-section {
		padding: clamp(5rem, 10vw, 9rem) 0;
		border-top: 1px solid var(--watch-rule);
	}

	.closing-section h2 {
		max-width: 18ch;
		font-size: clamp(3rem, 6vw, 6rem);
	}

	@media (max-width: 1000px) {
		.hero {
			grid-template-columns: 1fr;
			min-height: auto;
		}

		.hero-visual {
			min-height: 34rem;
		}

		.problem-section,
		.section-heading {
			grid-template-columns: 0.3fr 1fr;
		}

		.problem-section aside,
		.section-heading > p {
			grid-column: 2;
		}

		.firewatch-section {
			grid-template-columns: 1fr;
			gap: 2.5rem;
		}

		.firewatch-copy h2,
		.firewatch-copy > p {
			max-width: 42rem;
		}

		.signal-grid {
			grid-template-columns: repeat(2, 1fr);
		}

		.signal-grid article:nth-child(2) {
			border-right: 0;
		}

		.signal-grid article:nth-child(-n + 2) {
			border-bottom: 1px solid var(--watch-rule);
		}

		.workflow-section,
		.demo-section,
		.roadmap-section {
			grid-template-columns: 1fr;
		}

	}

	@media (max-width: 680px) {
		.hero,
		.problem-section,
		.signals-section,
		.workflow-section,
		.demo-section,
		.firewatch-section,
		.scope-section,
		.roadmap-section,
		.team-section,
		.closing-section {
			width: min(100vw - 2rem, 1320px);
		}

		.hero {
			padding-top: 3.25rem;
		}

		.hero h1 {
			font-size: clamp(3.3rem, 17vw, 5rem);
		}

		.hero-actions,
		.closing-section > div {
			align-items: stretch;
			flex-direction: column;
		}

		.availability,
		.scope-columns {
			grid-template-columns: 1fr;
		}

		.availability div + div {
			padding-left: 0;
			border-top: 1px solid var(--watch-rule);
			border-left: 0;
		}

		.hero-visual {
			min-height: 28rem;
		}

		.problem-section,
		.section-heading {
			grid-template-columns: 1fr;
		}

		.problem-section aside,
		.section-heading > p {
			grid-column: auto;
		}

		.signal-grid {
			grid-template-columns: 1fr;
		}

		.signal-grid article {
			min-height: 13rem;
			border-right: 0;
			border-bottom: 1px solid var(--watch-rule);
		}

		.signal-grid article:last-child {
			border-bottom: 0;
		}

		.signal-grid h3 {
			margin-top: 3rem;
		}

		.workflow-list li,
		.roadmap-section li {
			grid-template-columns: 2rem 1fr;
		}

		.workflow-list p,
		.roadmap-section li p {
			grid-column: 2;
		}

		.demo-console nav {
			grid-template-columns: 1fr;
		}

		.demo-console nav a {
			border-right: 0;
			border-bottom: 1px solid var(--watch-rule);
		}

		.demo-console nav a:last-child {
			border-bottom: 0;
		}

		.platform-lineage {
			gap: 0.75rem;
		}

		.platform-lineage strong {
			font-size: 1.35rem;
		}

		.platform-lineage em {
			font-size: 0.7rem;
		}

		.firewatch-facts div {
			grid-template-columns: 1fr;
			gap: 0.25rem;
		}

		.team-section {
			align-items: flex-start;
			flex-direction: column;
		}
	}
</style>
