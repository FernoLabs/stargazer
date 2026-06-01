<svelte:head>
	<title>FireWatch Score</title>
</svelte:head>

<div class="page-shell">
	<section class="panel intro">
		<h1>Score</h1>
		<p>
			FireWatch scores are a quick way to read project risk evidence. They are not an audit,
			a token rating, or a claim that a project is safe. The score tells you how much critical
			FireWatch evidence is currently attached to a watched project and whether that evidence
			has been reviewed.
		</p>
	</section>

	<section class="grade-table panel" aria-labelledby="grade-title">
		<div class="section-title">
			<h2 id="grade-title">Grades</h2>
			<p>Higher grades mean fewer unresolved critical signals in the FireWatch dataset.</p>
		</div>

		<div class="rows">
			<div class="grade-row">
				<strong>A</strong>
				<span>90-100</span>
				<p>No or very low unresolved critical evidence.</p>
			</div>
			<div class="grade-row">
				<strong>B</strong>
				<span>75-89</span>
				<p>Some critical evidence exists, but current unresolved weight is limited.</p>
			</div>
			<div class="grade-row">
				<strong>C</strong>
				<span>60-74</span>
				<p>Material critical evidence exists and should be inspected before relying on the project.</p>
			</div>
			<div class="grade-row">
				<strong>D</strong>
				<span>40-59</span>
				<p>Heavy unresolved critical evidence. The proof trail matters more than the letter.</p>
			</div>
			<div class="grade-row">
				<strong>F</strong>
				<span>0-39</span>
				<p>Severe unresolved evidence. Treat the project as requiring immediate review.</p>
			</div>
		</div>
	</section>

	<section class="grid">
		<article class="panel">
			<h2>What counts as proof</h2>
			<ul>
				<li>Transactions that changed a watched program, IDL, multisig, or admin path.</li>
				<li>Decoded instructions with critical or high-impact tags.</li>
				<li>Unknown instructions on protocols with parsed instructions.</li>
				<li>Amount-trigger and wallet movement signals when configured.</li>
				<li>Signer and governance history linked to the protocol.</li>
			</ul>
		</article>

			<article class="panel">
				<h2>How to read a project page</h2>
				<ol>
					<li>Start with the project grade and unresolved critical count.</li>
					<li>Open the evidence links to see the exact events behind the score.</li>
					<li>Use per-program proof counts to isolate where the evidence is concentrated.</li>
					<li>Use transaction links to verify source events on Solana Explorer.</li>
				</ol>
			</article>
	</section>

	<section class="panel">
		<h2>Limits</h2>
		<p>
			FireWatch can only score what it observes and understands. A clean score can still miss
			off-chain risk, economic design risk, hidden dependencies, private keys, oracle issues,
			or new behavior that has not been decoded yet. Projects should use the proof trail to
			explain the score and request deeper coverage when the current evidence is incomplete.
		</p>
		<div class="actions">
			<a href="/#protocols">Browse protocols</a>
			<a href="/events?scope=critical">Critical activity</a>
			<a href="mailto:contact@ferno.ag?subject=FireWatch%20score%20review">Contact us</a>
		</div>
	</section>
</div>

<style>
	.page-shell {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem 0 2.4rem;
		display: grid;
		gap: 1rem;
	}

	.panel {
		background: var(--fw-panel);
		border: 1px solid var(--fw-border);
		border-radius: 10px;
		padding: 1rem;
	}

	h1,
	h2,
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

	p,
	li {
		color: #5f6c79;
		line-height: 1.58;
	}

	.intro {
		display: grid;
		gap: 0.6rem;
	}

	.intro p {
		max-width: 72ch;
	}

	.section-title {
		display: grid;
		gap: 0.2rem;
	}

	.grade-table {
		display: grid;
		gap: 0.8rem;
	}

	.rows {
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

	.grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 1rem;
	}

	.grid .panel {
		display: grid;
		gap: 0.58rem;
		align-content: start;
	}

	ul,
	ol {
		display: grid;
		gap: 0.34rem;
		padding-left: 1.1rem;
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.8rem;
		margin-top: 0.8rem;
	}

	a {
		color: #5d4c3e;
		font-weight: 750;
		text-underline-offset: 0.14em;
	}

	@media (max-width: 820px) {
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
	}
</style>
