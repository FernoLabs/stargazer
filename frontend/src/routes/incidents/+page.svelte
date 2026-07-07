<script lang="ts">
	import { resolve } from '$app/paths';

	type IncidentOutcome = 'caught' | 'missed_then_improved' | 'planned';

	type TimelineLink = {
		label: string;
		href: string;
	};

	type AnchorReportLink = {
		label: string;
		href: `#${string}`;
		external?: false;
	};

	type ExternalReportLink = {
		label: string;
		href: `http${string}`;
		external: true;
	};

	type ReportLink = AnchorReportLink | ExternalReportLink;

	type IncidentReport = {
		id: string;
		title: string;
		date: string;
		outcome: IncidentOutcome;
		summary: string;
		detection: string;
		nextStep: string;
		timelineLinks: TimelineLink[];
		reportLinks: ReportLink[];
	};

	const incidents: IncidentReport[] = [
		{
			id: 'drift',
			title: 'Drift hack alert review',
			date: 'Historical incident',
			outcome: 'caught',
			summary:
				'FireWatch keeps Drift in the public incident notebook because it is the original reason this work exists: protocol risk needs a fast proof trail, not only a later explanation.',
			detection:
				'Timeline review should start from critical Drift events and then drill into exact transactions from each alert page.',
			nextStep:
				'Pin canonical Drift alert IDs once the public report links are selected, so this entry becomes fully reproducible from one click.',
			timelineLinks: [
				{ label: 'Critical timeline', href: '/events?protocol=drift&severity=critical' },
				{ label: 'All Drift activity', href: '/protocols/drift/activity' },
				{ label: 'Protocol page', href: '/protocols/drift' }
			],
			reportLinks: [{ label: 'Report anchor', href: '#drift' }]
		},
		{
			id: 'rent-drain',
			title: 'Kamino and Meteora user rent drain',
			date: 'Historical incident',
			outcome: 'caught',
			summary:
				'This case belongs here because it is closer to a user-facing exploit pattern than a simple admin event. The value is showing which movement alerts fired and where the user account proof begins.',
			detection:
				'Use the Kamino and Meteora timelines together, then keep the eventual report focused on the affected token account movements and signer path.',
			nextStep:
				'Replace the broad protocol links with exact alert IDs after the report chooses the canonical timeline.',
			timelineLinks: [
				{ label: 'Kamino critical timeline', href: '/events?protocol=kamino_lend&severity=critical' },
				{ label: 'Meteora critical timeline', href: '/events?protocol=meteora_dlmm&severity=critical' },
				{ label: 'Rent-drain search', href: '/events?search=rent%20drain' }
			],
			reportLinks: [{ label: 'Report anchor', href: '#rent-drain' }]
		},
		{
			id: 'raydium',
			title: 'Raydium legacy AMM V3 drain',
			date: 'June 10, 2026',
			outcome: 'missed_then_improved',
			summary:
				'Raydium reported unauthorized liquidity removal from deprecated legacy AMM V3 pools. Public reports describe about $1.34M drained from five old pools through a fake LP mint validation weakness.',
			detection:
				'Our first program-dormancy backfill did not alert on the candidate block. That proved the detector was too coarse: this incident needs dormant pool, vault, and token-account reserve-drain coverage.',
			nextStep:
				'Keep Raydium as the post-mortem entry for the new trigger work, then add the backfill proof here once the pool/vault token-account detector fires on the historical block.',
			timelineLinks: [
				{ label: 'Raydium activity', href: '/events?protocol=raydium' },
				{ label: 'Dormancy search', href: '/events?protocol=raydium&search=dormant' },
				{ label: 'Protocol page', href: '/protocols/raydium' }
			],
			reportLinks: [
				{
					label: 'Raydium public note',
					href: 'https://x.com/0xINFRA/status/2064738005697384476',
					external: true
				},
				{
					label: 'Incident summary',
					href: 'https://www.ccn.com/news/crypto/raydium-exploit-legacy-pools-solana/',
					external: true
				}
			]
		}
	];

	const pageIndex = incidents.map((incident) => ({
		href: `#${incident.id}`,
		label: incident.title
	}));

	function outcomeLabel(outcome: IncidentOutcome) {
		if (outcome === 'caught') return 'Caught';
		if (outcome === 'missed_then_improved') return 'Missed, improving';
		return 'Planned';
	}

	function outcomeClass(outcome: IncidentOutcome) {
		if (outcome === 'caught') return 'outcome-caught';
		if (outcome === 'missed_then_improved') return 'outcome-improving';
		return 'outcome-planned';
	}

</script>

<svelte:head>
	<title>Incident Log | FireWatch</title>
	<meta
		name="description"
		content="FireWatch incident notebook with alert timelines, short reports, and post-mortems for Solana protocol risk coverage."
	/>
</svelte:head>

<div class="page-shell">
	<div class="docs-layout">
		<aside class="doc-index" aria-label="Incident page index">
			<div class="index-shell">
				<h2>Incident Log</h2>
				<nav>
					{#each pageIndex as item (item.href)}
						<a href={resolve(`/incidents${item.href}` as any)}>{item.label}</a>
					{/each}
				</nav>
			</div>
		</aside>

		<main class="doc-main">
			<section class="doc-section intro">
				<h1>Incident Log</h1>
				<p>
					A public notebook for incidents FireWatch caught, incidents we missed, and the detector work
					that came out of each review. Each entry should point to alert timelines first, then the short
					report or post-mortem.
				</p>
			</section>

			<section class="doc-section">
				<h2>Coverage Table</h2>
					<div class="incident-table" role="list" aria-label="Incident coverage table">
						{#each incidents as incident (incident.id)}
							{@const rowReportLink = incident.reportLinks[0]}
							<div class="incident-row" role="listitem">
								<div>
									<a
										class="incident-title"
										href={resolve(`/incidents#${incident.id}` as any)}
									>
										{incident.title}
									</a>
									<p>{incident.date}</p>
								</div>
								<strong class={outcomeClass(incident.outcome)}>{outcomeLabel(incident.outcome)}</strong>
								<div class="row-links">
									<a href={resolve(incident.timelineLinks[0].href as any)}>
										{incident.timelineLinks[0].label}
									</a>
									{#if rowReportLink.external}
										<a href={rowReportLink.href} rel="noreferrer" target="_blank">
											{rowReportLink.label}
										</a>
									{:else}
										<a href={resolve(`/incidents${rowReportLink.href}` as any)}>
											{rowReportLink.label}
										</a>
									{/if}
								</div>
						</div>
					{/each}
				</div>
			</section>

			{#each incidents as incident (incident.id)}
				<section class="doc-section report" id={incident.id}>
					<div class="report-heading">
						<div>
							<h2>{incident.title}</h2>
							<p>{incident.date}</p>
						</div>
						<strong class={outcomeClass(incident.outcome)}>{outcomeLabel(incident.outcome)}</strong>
					</div>

					<div class="report-body">
						<div>
							<h3>Short Report</h3>
							<p>{incident.summary}</p>
						</div>
						<div>
							<h3>Detection Note</h3>
							<p>{incident.detection}</p>
						</div>
						<div>
							<h3>Next Proof</h3>
							<p>{incident.nextStep}</p>
						</div>
					</div>

					<div class="link-groups">
						<div>
							<h3>Alert Timeline</h3>
							<div class="actions">
								{#each incident.timelineLinks as link (link.href)}
									<a href={resolve(link.href as any)}>{link.label}</a>
								{/each}
							</div>
						</div>
						<div>
							<h3>Report Links</h3>
							<div class="actions">
								{#each incident.reportLinks as link (link.href)}
									{#if link.external}
										<a href={link.href} rel="noreferrer" target="_blank">
											{link.label}
										</a>
									{:else}
										<a href={resolve(`/incidents${link.href}` as any)}>
											{link.label}
										</a>
									{/if}
								{/each}
							</div>
						</div>
					</div>
				</section>
			{/each}
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
		gap: 0.74rem;
		scroll-margin-top: 6rem;
	}

	.intro {
		gap: 0.68rem;
	}

	.incident-table {
		display: grid;
		border: 1px solid var(--fw-border);
		border-radius: 8px;
		overflow: hidden;
	}

	.incident-row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 148px minmax(220px, 0.76fr);
		gap: 0.8rem;
		align-items: center;
		padding: 0.78rem 0.85rem;
		border-bottom: 1px solid var(--fw-border);
		background: #fff;
	}

	.incident-row:last-child {
		border-bottom: 0;
	}

	.incident-title {
		color: var(--fw-ink);
		font-weight: 780;
		text-decoration: none;
	}

	.incident-title:hover {
		color: #8d421a;
	}

	.row-links,
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.7rem;
		align-items: center;
	}

	.report {
		gap: 0.82rem;
	}

	.report-heading {
		display: flex;
		justify-content: space-between;
		gap: 1rem;
		align-items: start;
	}

	.report-body {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 0.9rem;
	}

	.link-groups {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 0.9rem;
		padding-top: 0.1rem;
	}

	.report-body > div,
	.link-groups > div {
		display: grid;
		gap: 0.34rem;
		align-content: start;
	}

	.outcome-caught,
	.outcome-improving,
	.outcome-planned {
		font-size: 0.82rem;
		line-height: 1.25;
	}

	.outcome-caught {
		color: #1f7a4d;
	}

	.outcome-improving {
		color: #9a4b16;
	}

	.outcome-planned {
		color: #556473;
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
	p {
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

	p {
		color: #5f6c79;
		line-height: 1.58;
	}

	a {
		color: #5d4c3e;
		font-weight: 750;
		text-underline-offset: 0.14em;
	}

	@media (max-width: 900px) {
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

		.incident-row,
		.report-body,
		.link-groups {
			grid-template-columns: 1fr;
		}

		.report-heading {
			display: grid;
			gap: 0.42rem;
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

		.doc-section {
			padding: 0.9rem;
		}
	}
</style>
