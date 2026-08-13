<script lang="ts">
	import { resolve } from '$app/paths';
	import PipelineFlow from '$lib/components/docs/PipelineFlow.svelte';
	import StatusNote from '$lib/components/docs/StatusNote.svelte';

	const archiveSteps = [
		{
			label: 'Local Galexie archive',
			detail:
				'Closed ledger partitions are kept on the NAS so large historical passes are reproducible and do not depend on repeated RPC retrieval.',
			status: 'Implemented'
		},
		{
			label: 'Bounded Rust decoding',
			detail:
				'Compressed XDR streams directly into capped workers. Ledgers may decode concurrently, but validation and database writes remain in canonical order.',
			status: 'Implemented'
		},
		{
			label: 'Compact reverse index',
			detail:
				'Contract transactions, exact wallet-to-contract edges, protocol attribution, and executable history are committed to a resumable SQLite database.',
			status: 'Implemented'
		},
		{
			label: 'Serving snapshot',
			detail:
				'A separate bounded-memory finalizer derives wallet activity and leaderboard orderings, then publishes the completed database with an atomic rename.',
			status: 'Implemented'
		},
		{
			label: 'Read API and demo',
			detail:
				'The API opens the evidence and serving databases read-only, validates their coverage, and exposes bounded wallet, protocol, upgrade, and ranking queries.',
			status: 'Implemented'
		}
	] as const;

	const backfillSteps = [
		{
			label: 'Define the new fact',
			detail: 'Add the parser, schema contract, provenance rules, and representative regression fixtures.',
			status: 'Planned'
		},
		{
			label: 'Replay the offline archive',
			detail:
				'Run the new indexer against retained local XDR with notification delivery disabled and durable checkpoints enabled.',
			status: 'Planned'
		},
		{
			label: 'Validate and publish',
			detail:
				'Compare coverage and invariants, then atomically expose the completed tables or serving snapshot.',
			status: 'Planned'
		},
		{
			label: 'Continue from the live cursor',
			detail:
				'Hand the same normalized ledger input to the optimized indexer and use the archive path to recover any gap.',
			status: 'Planned'
		}
	] as const;

	const readingMap = [
		{
			href: '/docs/architecture',
			label: 'Architecture',
			detail: 'Component boundaries, shared invariants, and the archive-to-live handoff.'
		},
		{
			href: '/docs/archive-indexer',
			label: 'Archive indexer',
			detail: 'How the NAS archive is discovered, decoded, validated, and indexed efficiently.'
		},
		{
			href: '/docs/live-ingestion',
			label: 'Live ingestion',
			detail: 'The target live tail, checkpointing, and archive-backed gap recovery.'
		},
		{
			href: '/docs/backfill',
			label: 'Backfill workflow',
			detail: 'How a new parser is tested on history before it joins the live path.'
		},
		{
			href: '/docs/data-model',
			label: 'Data model',
			detail: 'Evidence, reverse-index relationships, executable versions, and serving views.'
		},
		{
			href: '/docs/operations',
			label: 'Operations',
			detail: 'Continuity invariants, health signals, publication, recovery, and failure handling.'
		},
		{
			href: '/docs/roadmap',
			label: 'Roadmap',
			detail: 'More protocol coverage, signal sources, delivery channels, and operating capacity.'
		}
	] as const;
</script>

<svelte:head>
	<title>Technical documentation · Stargazer</title>
	<meta
		name="description"
		content="How Stargazer turns a local Stellar ledger archive into a compact protocol intelligence index, then extends the same processing model to live ingestion and repeatable backfills."
	/>
	<link rel="canonical" href="https://stargazer.ferno.ag/docs" />
</svelte:head>

<article class="doc-page">
	<header class="doc-header">
		<h1>How Stargazer processes Stellar data</h1>
		<p class="doc-summary">
			Stargazer keeps the raw ledger evidence close to the indexer, derives compact databases for
			product queries, and is designed to reuse the same deterministic processing core for new
			ledgers and historical replays.
		</p>
	</header>

	<section class="doc-section" aria-labelledby="status-heading">
		<h2 id="status-heading">Current system and target architecture</h2>
		<p>
			The archive-backed demo path is implemented. Continuous live ingestion and broader off-chain
			signal collection are the next operating layers; this documentation labels those boundaries
			explicitly.
		</p>
		<div class="doc-status-list">
			<StatusNote status="Implemented" title="Archive evidence, optimized indexing, and read access">
				<p>
					The Rust reader validates local Galexie ledger batches, builds a resumable compact index, and
					serves it through a bounded read-only API. A separate finalizer publishes wallet activity in an
					atomic serving snapshot.
				</p>
			</StatusNote>
			<StatusNote status="In progress" title="Continuous ledger acquisition">
				<p>
					The live acquisition boundary and source strategy are defined, but the production live service
					is not wired yet. It will emit the same ledger metadata consumed by the archive reader and use
					the local archive or archive RPC for gap recovery.
				</p>
			</StatusNote>
			<StatusNote status="Planned" title="Broader protocol intelligence">
				<p>
					Additional protocol adapters, multisig and controller monitoring, governance, repositories,
					SDKs, packages, and release sources will expand the monitored surface without changing the
					evidence and backfill model.
				</p>
			</StatusNote>
		</div>
	</section>

	<section class="doc-section" aria-labelledby="local-first-heading">
		<h2 id="local-first-heading">What local-first means</h2>
		<p>
			The local archive is a retained Galexie copy on the NAS, alongside local index databases and
			benchmarks. It lets engineering runs use stable inputs, resume without redownloading history,
			and preserve an exact path back to the compressed ledger evidence.
		</p>
		<p>
			It is not a Stellar validating node and it does not publish a network history archive. The
			application consumes complete <code>LedgerCloseMeta</code> records exported by Galexie; its own
			databases contain derived, searchable facts.
		</p>
	</section>

	<section class="doc-section" aria-labelledby="pipeline-heading">
		<h2 id="pipeline-heading">Archive-backed product path</h2>
		<p>
			The bulk path separates immutable evidence, ingestion-optimized storage, and query-optimized
			serving data. That keeps the large historical write path lean and the public API bounded.
		</p>
		<PipelineFlow steps={archiveSteps} />
	</section>

	<section class="doc-section" aria-labelledby="shared-core-heading">
		<h2 id="shared-core-heading">One processing contract, two time horizons</h2>
		<div class="doc-split">
			<div>
				<h3>Historical path</h3>
				<p>
					Closed archive partitions are processed oldest first. Decoding can run concurrently within a
					fixed memory budget, while commits preserve ledger order and advance one durable high-water mark.
				</p>
			</div>
			<div>
				<h3>Live path</h3>
				<p>
					RPC or Captive Core will supply newly confirmed ledger metadata to the same normalization and
					indexing boundary. If the live cursor falls behind, retained archive data fills the missing range
					before the tail resumes.
			</p>
			</div>
		</div>
	</section>

	<section class="doc-section" aria-labelledby="backfill-heading">
		<h2 id="backfill-heading">Adding an indexer starts with history</h2>
		<p>
			A new fact extractor should prove that it can replay deterministically before it is trusted with
			live alerts. The offline archive is therefore part of the development loop, not only a bootstrap
			mechanism.
		</p>
		<PipelineFlow steps={backfillSteps} />
	</section>

	<section class="doc-section" aria-labelledby="reading-heading">
		<h2 id="reading-heading">Documentation map</h2>
		<p>Use the component pages for implementation details, operating boundaries, and roadmap scope.</p>
		<nav class="doc-link-list" aria-label="Documentation pages">
			{#each readingMap as item (item.href)}
				<a href={resolve(item.href)}>
					<strong>{item.label}</strong>
					<span>{item.detail}</span>
					<span aria-hidden="true">→</span>
				</a>
			{/each}
		</nav>
	</section>
</article>
