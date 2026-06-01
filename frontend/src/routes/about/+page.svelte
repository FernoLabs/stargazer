<script lang="ts">
	type GradeRow = {
		grade: string;
		range: string;
		note: string;
	};

	const gradeRows: GradeRow[] = [
		{
			grade: 'A',
			range: '90-100',
			note: 'No or very low unresolved critical evidence.'
		},
		{
			grade: 'B',
			range: '75-89',
			note: 'Some critical evidence exists, but current unresolved weight is limited.'
		},
		{
			grade: 'C',
			range: '60-74',
			note: 'Material critical evidence exists and should be inspected before relying on the project.'
		},
		{
			grade: 'D',
			range: '40-59',
			note: 'Heavy unresolved critical evidence. The proof trail matters more than the letter.'
		},
		{
			grade: 'F',
			range: '0-39',
			note: 'Severe unresolved evidence. Treat the project as requiring immediate review.'
		}
	];

	const pageIndex = [
		{ href: '#story', label: 'Why FireWatch' },
		{ href: '#tracking', label: 'What We Track' },
		{ href: '#score', label: 'Score Model' },
		{ href: '#grades', label: 'Grade Bands' },
		{ href: '#proof', label: 'Proof Sources' },
		{ href: '#read-score', label: 'How To Read Score' },
		{ href: '#limits', label: 'Score Limits' },
		{ href: '#infra', label: 'Infra' },
		{ href: '#oss', label: 'Open Source' }
	];
</script>

<svelte:head>
	<title>About FireWatch</title>
</svelte:head>

<div class="page-shell">
	<div class="docs-layout">
		<aside class="doc-index" aria-label="About page index">
			<div class="index-shell">
				<h2>On This Page</h2>
				<nav>
					{#each pageIndex as item (item.href)}
						<a href={item.href}>{item.label}</a>
					{/each}
				</nav>
			</div>
		</aside>

		<main class="doc-main">
			<section class="doc-section intro" id="story">
				<h1>About FireWatch</h1>
				<p>
					After the Drift incident era, Ferno decided to open its monitoring technology and make it
					available to anyone operating on Solana. DeFi can feel scary because teams watch portfolio
					moves all day while protocol risk is often treated as unavoidable.
				</p>
				<p>
					FireWatch is built on a different assumption: protocol risk is not fully avoidable, but a lot
					can be prevented when evidence is fast, structured, and actionable. That is exactly what
					FireWatch is for.
				</p>
			</section>

			<section class="doc-section" id="tracking">
				<h2>What We Track</h2>
				<div class="grid">
					<article class="sub-panel">
						<h3>Program and governance signals</h3>
						<ul>
							<li>Program deploy, upgrade, finalize, and loader migration events.</li>
							<li>IDL initialize, update, authority change, immutable, resize, and close actions.</li>
							<li>Curated admin and high-impact instruction paths when known.</li>
							<li>Unknown instruction alerts on protocols with parsed instructions.</li>
						</ul>
					</article>

					<article class="sub-panel">
						<h3>Operational risk signals</h3>
						<ul>
							<li>Squads proposal lifecycle and vault/config execution traces.</li>
							<li>Wallet and amount-trigger alerts for configured monitors and thresholds.</li>
							<li>.sol domain ownership and update events tied to monitored entities.</li>
							<li>Protocol-linked event history with proof-first event pages.</li>
						</ul>
					</article>
				</div>
			</section>

			<section class="doc-section" id="score">
				<h2>Score Model</h2>
				<p>
					FireWatch score is not an audit badge, token rating, or guarantee of safety. It is a live
					evidence summary showing how much critical proof is attached to a watched project and whether
					that evidence has been reviewed.
				</p>
			</section>

			<section class="doc-section" id="grades">
				<h2>Grade Bands</h2>
				<div class="grade-table" role="list" aria-label="FireWatch score grade bands">
					{#each gradeRows as row (row.grade)}
						<div class="grade-row" role="listitem">
							<strong>{row.grade}</strong>
							<span>{row.range}</span>
							<p>{row.note}</p>
						</div>
					{/each}
				</div>
			</section>

			<section class="doc-section" id="proof">
				<h2>What Counts As Proof</h2>
				<ul>
					<li>Transactions that changed a watched program, IDL, multisig, or admin path.</li>
					<li>Decoded instructions with critical or high-impact tags.</li>
					<li>Unknown instructions on protocols with parsed instructions.</li>
					<li>Amount-trigger and wallet movement signals when configured.</li>
					<li>Signer and governance history linked to the protocol.</li>
				</ul>
			</section>

			<section class="doc-section" id="read-score">
				<h2>How To Read A Project Score</h2>
				<ol>
					<li>Start with grade and unresolved critical count.</li>
					<li>Open evidence links to inspect exact events behind the score.</li>
					<li>Use per-program proof counts to isolate concentration of risk.</li>
					<li>Verify source events on Solana Explorer from linked transactions.</li>
				</ol>
			</section>

			<section class="doc-section" id="limits">
				<h2>Score Limits</h2>
				<p>
					FireWatch can only score what it observes and understands. A clean score may still miss
					off-chain risk, economic design risk, hidden dependencies, key management issues, oracle
					weakness, or brand-new behavior that has not been decoded yet.
				</p>
				<div class="actions">
					<a href="/events?scope=critical">Critical activity</a>
					<a href="/protocols">Browse protocols</a>
				</div>
			</section>

			<section class="doc-section" id="infra">
				<h2>Infra</h2>
				<p>
					For on-demand response workflows, FireWatch can run on Ferno’s dedicated stack: DoubleZero
					shred stream ingestion, custom send paths, and a multi-tenant transaction sender targeting
					near-zero slot delay inclusion.
				</p>
			</section>

			<section class="doc-section" id="oss">
				<h2>Open Source And Contribution</h2>
				<p>
					FireWatch is open source. We want protocol teams and operators to improve coverage with us.
				</p>
				<ol>
					<li>
						Add or refine curated protocol definitions in
						<code>backend/src/protocols.rs</code> (program IDs, instruction coverage, critical paths).
					</li>
					<li>
						If needed, update platform source data in
						<code>backend/data/platform_registry_snapshot.json</code>.
					</li>
					<li>
						Run checks before opening a PR:
						<code>cd backend && cargo test</code>,
						<code>cd frontend && npm run check</code>.
					</li>
				</ol>
				<div class="actions">
					<a href="https://github.com/FernoLabs/firewatch" rel="noreferrer" target="_blank">Source code</a>
					<a href="https://github.com/FernoLabs/firewatch/issues/new/choose" rel="noreferrer" target="_blank">
						Feature request
					</a>
					<a href="mailto:contact@ferno.ag?subject=FireWatch%20feature%20request">Contact us</a>
				</div>
			</section>
		</main>
	</div>
</div>

<style>
	.page-shell {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem 0 2.4rem;
	}

	.docs-layout {
		display: grid;
		grid-template-columns: minmax(210px, 240px) minmax(0, 1fr);
		gap: 1rem;
		align-items: start;
	}

	.doc-main {
		display: grid;
		gap: 0.9rem;
	}

	.doc-section {
		background: var(--fw-panel);
		border: 1px solid var(--fw-border);
		border-radius: 10px;
		padding: 1rem;
		display: grid;
		gap: 0.6rem;
		scroll-margin-top: 6rem;
	}

	.intro {
		gap: 0.68rem;
	}

	.sub-panel {
		background: #fbfaf8;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 8px;
		padding: 0.82rem;
		display: grid;
		gap: 0.42rem;
	}

	.grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 0.8rem;
	}

	.grade-table {
		display: grid;
		border: 1px solid var(--fw-border);
		border-radius: 8px;
		overflow: hidden;
	}

	.grade-row {
		display: grid;
		grid-template-columns: 56px 80px minmax(0, 1fr);
		gap: 0.8rem;
		align-items: center;
		padding: 0.72rem 0.8rem;
		border-bottom: 1px solid var(--fw-border);
		background: #fff;
	}

	.grade-row:last-child {
		border-bottom: 0;
	}

	.grade-row strong {
		font-size: 1.1rem;
	}

	.grade-row span {
		color: #62707d;
		font-size: 0.82rem;
		font-weight: 750;
	}

	.doc-index {
		position: sticky;
		top: 5.8rem;
	}

	.index-shell {
		background: #fff;
		border: 1px solid var(--fw-border);
		border-radius: 10px;
		padding: 0.9rem 0.85rem;
		display: grid;
		gap: 0.56rem;
	}

	.index-shell h2 {
		font-size: 0.88rem;
		line-height: 1.2;
		margin: 0;
	}

	.index-shell nav {
		display: grid;
		gap: 0.34rem;
	}

	.index-shell a {
		font-size: 0.79rem;
		font-weight: 700;
		color: #5f6c79;
		text-decoration: none;
	}

	.index-shell a:hover {
		color: #8d421a;
	}

	h1,
	h2,
	h3,
	p,
	ul,
	ol {
		margin: 0;
	}

	h1 {
		font-size: 2rem;
		line-height: 1.05;
		letter-spacing: 0;
	}

	h2 {
		font-size: 1.04rem;
		line-height: 1.25;
	}

	h3 {
		font-size: 0.92rem;
		line-height: 1.3;
	}

	p,
	li {
		color: #5f6c79;
		line-height: 1.58;
	}

	ul,
	ol {
		display: grid;
		gap: 0.32rem;
		padding-left: 1.1rem;
	}

	code {
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
		font-size: 0.8rem;
		overflow-wrap: anywhere;
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.8rem;
		margin-top: 0.15rem;
	}

	a {
		color: #5d4c3e;
		font-weight: 750;
		text-underline-offset: 0.14em;
	}

	@media (max-width: 820px) {
		.docs-layout {
			grid-template-columns: 1fr;
		}

		.doc-index {
			position: static;
			order: -1;
		}

		.index-shell nav {
			grid-template-columns: repeat(2, minmax(0, 1fr));
			gap: 0.4rem 0.8rem;
		}

		.grid {
			grid-template-columns: 1fr;
		}

		.grade-row {
			grid-template-columns: 44px minmax(0, 1fr);
		}

		.grade-row p {
			grid-column: 1 / -1;
		}
	}

	@media (max-width: 720px) {
		.page-shell {
			width: min(100vw - 1rem, var(--fw-page-max));
			padding-top: 0.8rem;
		}

		.index-shell nav {
			grid-template-columns: 1fr;
		}
	}
</style>
