<script lang="ts">
	import PipelineFlow from '$lib/components/docs/PipelineFlow.svelte';
	import StatusNote from '$lib/components/docs/StatusNote.svelte';

	const archivePath = [
		{
			label: 'Local Galexie archive',
			detail: 'Immutable per-ledger XDR retained on the NAS.',
			status: 'Implemented'
		},
		{
			label: 'Bounded decode',
			detail: 'Streaming zstd/XDR decode with a fixed worker and queue budget.',
			status: 'Implemented'
		},
		{
			label: 'Indexing core',
			detail: 'Deterministic facts, exact relationships, versions, and ordered commits.',
			status: 'Implemented'
		},
		{
			label: 'SQLite evidence stores',
			detail: 'Compact history plus an atomically published wallet serving snapshot.',
			status: 'Implemented'
		},
		{
			label: 'Read API and demo',
			detail: 'Bounded read-only queries served through the Stargazer product.',
			status: 'Implemented'
		}
	] as const;

	const livePath = [
		{
			label: 'RPC or Captive Core',
			detail: 'Acquire recent and newly closed LedgerCloseMeta records.',
			status: 'Planned'
		},
		{
			label: 'Normalized ledger input',
			detail: 'Give archive and live sources one versioned processing boundary.',
			status: 'In progress'
		},
		{
			label: 'Shared indexing modules',
			detail: 'Run the same tested fact and protocol logic used during backfill.',
			status: 'Planned'
		},
		{
			label: 'Atomic checkpoint',
			detail: 'Advance the live watermark only after facts and findings commit.',
			status: 'Planned'
		},
		{
			label: 'Alert delivery',
			detail: 'Dispatch committed findings to subscribed users after policy checks.',
			status: 'Planned'
		}
	] as const;
</script>

<svelte:head>
	<title>System architecture · Stargazer documentation</title>
	<meta
		name="description"
		content="The implemented and target architecture for Stargazer's local-first Stellar archive, indexers, databases, live ingestion, APIs, and alert delivery."
	/>
	<link rel="canonical" href="https://stargazer.ferno.ag/docs/architecture" />
</svelte:head>

<article class="doc-page">
	<header class="doc-hero">
		<p class="doc-kicker">System / Architecture</p>
		<h1>One evidence pipeline, two ingestion paths.</h1>
		<p class="doc-lede">
			Stargazer starts with an offline, locally retained Stellar archive. That archive is where
			indexing logic is developed, benchmarked, and replayed. The live service will feed newly closed
			ledgers into the same logical processing boundary so historical and real-time results do not
			drift apart.
		</p>
	</header>

	<StatusNote status="In progress" title="Architecture boundary">
		The archive reader, compact database, wallet finalizer, read API, and governance watcher exist
		today. Continuous ledger ingestion, a pluggable shared module runner, and event delivery are the
		target architecture and are not presented as production services.
	</StatusNote>

	<section class="doc-section" id="archive-path">
		<div class="doc-section-heading">
			<p class="doc-kicker">01 / Implemented path</p>
			<h2>Archive replay produces the evidence database.</h2>
		</div>
		<PipelineFlow steps={archivePath} />
		<p>
			The raw Galexie files remain the durable source of truth. Derived databases are disposable: they
			can be deleted, rebuilt from the archive, and compared across parser versions without downloading
			the source data again.
		</p>
	</section>

	<section class="doc-section" id="live-path">
		<div class="doc-section-heading">
			<p class="doc-kicker">02 / Target path</p>
			<h2>Live ingestion joins before indexing logic.</h2>
		</div>
		<PipelineFlow steps={livePath} />
		<p>
			Only acquisition should know whether a ledger came from the local archive, an RPC backend, or
			Captive Core. Once decoded, the same network coordinates, fact extraction, protocol modules, and
			rules should process it. This is the main architectural step still to implement.
		</p>
	</section>

	<section class="doc-section" id="boundaries">
		<div class="doc-section-heading">
			<p class="doc-kicker">03 / Trust boundaries</p>
			<h2>Each service has one reason to write.</h2>
		</div>
		<div class="table-scroll">
			<table>
				<thead>
					<tr>
						<th>Boundary</th>
						<th>Responsibility</th>
						<th>Write access</th>
						<th>Status</th>
					</tr>
				</thead>
				<tbody>
					<tr>
						<td>Archive acquisition</td>
						<td>Retain immutable Galexie ledger objects and completion markers.</td>
						<td>Raw archive only</td>
						<td>Implemented</td>
					</tr>
					<tr>
						<td>Archive indexer</td>
						<td>Validate continuity, extract facts, and append ordered batches.</td>
						<td>Compact evidence DB</td>
						<td>Implemented</td>
					</tr>
					<tr>
						<td>Wallet finalizer</td>
						<td>Build account-oriented aggregates without mutating the evidence store.</td>
						<td>New serving DB</td>
						<td>Implemented</td>
					</tr>
					<tr>
						<td>Read API</td>
						<td>Validate schemas and answer bounded, deadline-limited queries.</td>
						<td>None</td>
						<td>Implemented</td>
					</tr>
					<tr>
						<td>Alert control</td>
						<td>Pair Telegram identities and store user policy separately.</td>
						<td>Dedicated control DB</td>
						<td>Pairing implemented</td>
					</tr>
					<tr>
						<td>Live ledger service</td>
						<td>Acquire, checkpoint, recover gaps, and submit normalized ledgers.</td>
						<td>Checkpoint and event store</td>
						<td>Planned</td>
					</tr>
				</tbody>
			</table>
		</div>
	</section>

	<section class="doc-section" id="deployment">
		<div class="doc-section-heading">
			<p class="doc-kicker">04 / Deployment</p>
			<h2>Storage stays local; the public edge stays narrow.</h2>
		</div>
		<div class="architecture-lanes">
			<section>
				<h3>NAS / private network</h3>
				<ul>
					<li>Raw compressed Stellar archive</li>
					<li>Rust archive reader and wallet finalizer</li>
					<li>Compact and serving SQLite databases</li>
					<li>Read-only API and governance snapshots</li>
				</ul>
			</section>
			<section>
				<h3>Cloudflare / public edge</h3>
				<ul>
					<li>SvelteKit website and server-rendered demo</li>
					<li>Allowlisted GET/HEAD gateway routes</li>
					<li>Private service binding to the NAS tunnel</li>
					<li>No public database or archive filesystem access</li>
				</ul>
			</section>
		</div>
	</section>

	<section class="doc-section" id="principles">
		<div class="doc-section-heading">
			<p class="doc-kicker">05 / Design rules</p>
			<h2>Properties the implementation must preserve.</h2>
		</div>
		<ol class="doc-numbered-list">
			<li>
				<strong>Evidence is replayable.</strong>
				<span>Every derived fact points back to a network, ledger, transaction, and local archive path.</span>
			</li>
			<li>
				<strong>Commits stay ordered.</strong>
				<span>Decode may run concurrently; database state advances in deterministic ledger order.</span>
			</li>
			<li>
				<strong>Backfill is silent.</strong>
				<span>Historical findings are retained for review but never delivered as new live alerts.</span>
			</li>
			<li>
				<strong>Unknown remains unknown.</strong>
				<span>Unattributed contracts keep their on-chain address instead of receiving an invented label.</span>
			</li>
			<li>
				<strong>Source and product state are separate.</strong>
				<span>The read path cannot mutate evidence, and alert policy cannot rewrite chain facts.</span>
			</li>
		</ol>
	</section>
</article>
