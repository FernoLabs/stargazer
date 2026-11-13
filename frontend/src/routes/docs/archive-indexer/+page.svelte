<script lang="ts">
	import PipelineFlow from '$lib/components/docs/PipelineFlow.svelte';
	import StatusNote from '$lib/components/docs/StatusNote.svelte';

	const pipeline = [
		{
			label: 'Mirror partitions',
			detail: 'Download closed 64,000-ledger partitions from the public Galexie lake.',
			status: 'Implemented'
		},
		{
			label: 'Walk in order',
			detail: 'Hold only one partition of paths in memory and yield ledgers sequentially.',
			status: 'Implemented'
		},
		{
			label: 'Decode concurrently',
			detail: 'Stream zstd into bounded XDR workers without materializing an uncompressed blob.',
			status: 'Implemented'
		},
		{
			label: 'Extract facts',
			detail: 'Keep contract transactions, exact wallet edges, protocol roots, and versions.',
			status: 'Implemented'
		},
		{
			label: 'Commit batches',
			detail: 'Write ledgers in chain order and advance only after the SQLite transaction commits.',
			status: 'Implemented'
		},
		{
			label: 'Finalize for reads',
			detail: 'Build account-oriented aggregates beside the immutable evidence database.',
			status: 'Implemented'
		}
	] as const;
</script>

<svelte:head>
	<title>Archive indexer · Stargazer documentation</title>
	<meta
		name="description"
		content="How Stargazer mirrors Stellar Galexie history locally and turns it into a resumable, memory-efficient SQLite evidence index."
	/>
	<link rel="canonical" href="https://stargazer.ferno.ag/docs/archive-indexer" />
</svelte:head>

<article class="doc-page">
	<header class="doc-hero">
		<p class="doc-kicker">Pipeline / Archive indexer</p>
		<h1>Local history is the development and recovery surface.</h1>
		<p class="doc-lede">
			Stargazer mirrors Stellar's Galexie ledger data onto local storage, then replays it with a
			bounded Rust pipeline. Parsing experiments, schema changes, new protocol modules, and incident
			research can run repeatedly without depending on remote RPC retention.
		</p>
	</header>

	<StatusNote status="Implemented" title="Current implementation">
		The compact archive indexer is the source of the deployed demo. It processes closed partitions in
		order, resumes an existing contiguous range, and deliberately excludes the changing tip partition.
	</StatusNote>

	<section class="doc-section" id="pipeline">
		<div class="doc-section-heading">
			<p class="doc-kicker">01 / Data path</p>
			<h2>Compressed ledger to queryable evidence.</h2>
		</div>
		<PipelineFlow steps={pipeline} />
	</section>

	<section class="doc-section" id="local-first">
		<div class="doc-section-heading">
			<p class="doc-kicker">02 / Local-first archive</p>
			<h2>A mirror, not a validator.</h2>
		</div>
		<p>
			The archive host keeps a local mirror of Galexie <code>LedgerCloseMeta</code> objects. It does not
			participate in consensus and should not be described as a Stellar validator. The downloader works
			one 64,000-ledger partition at a time, reuses existing files by size, and records completion only
			for closed partitions.
		</p>
		<dl class="definition-grid">
			<div>
				<dt>Raw evidence</dt>
				<dd>Compressed <code>.xdr.zst</code> objects stay immutable and independently replayable.</dd>
			</div>
			<div>
				<dt>Retention window</dt>
				<dd>The current runner targets an approximate recent-year window using ledger cadence.</dd>
			</div>
			<div>
				<dt>Tip handling</dt>
				<dd>The newest partition is synchronized again later and never treated as closed evidence.</dd>
			</div>
			<div>
				<dt>Concurrency guard</dt>
				<dd>Filesystem locks prevent duplicate downloaders and duplicate index writers.</dd>
			</div>
		</dl>
	</section>

	<section class="doc-section" id="memory">
		<div class="doc-section-heading">
			<p class="doc-kicker">03 / Resource model</p>
			<h2>Bound memory before chasing throughput.</h2>
		</div>
		<div class="table-scroll">
			<table>
				<thead>
					<tr>
						<th>Pressure point</th>
						<th>Control</th>
						<th>Current default or behavior</th>
					</tr>
				</thead>
				<tbody>
					<tr>
						<td>Directory discovery</td>
						<td>Partition-scoped walker</td>
						<td>At most one partition of paths, normally 64,000 files</td>
					</tr>
					<tr>
						<td>Decompression</td>
						<td>Streaming zstd reader</td>
						<td>No decompressed ledger-sized buffer</td>
					</tr>
					<tr>
						<td>Malformed input</td>
						<td>Compressed, decoded, depth, and zstd-window caps</td>
						<td>Reject before unbounded allocation</td>
					</tr>
					<tr>
						<td>Parallel decode</td>
						<td>Fixed worker pool and hard decode-ahead limit</td>
						<td>Two workers, four in-flight ledgers in the archive runner</td>
					</tr>
					<tr>
						<td>Database writes</td>
						<td>One ordered writer with atomic batches</td>
						<td>2,000 ledgers per compact batch</td>
					</tr>
					<tr>
						<td>Wallet ranking</td>
						<td>256 disk-backed shards, one aggregated at a time</td>
						<td>Designed to stay bounded on an 8 GiB archive host</td>
					</tr>
				</tbody>
			</table>
		</div>
		<p>
			A representative 1,000-ledger local soak measured 56.6 ledgers per second with two decoders;
			that is an engineering benchmark, not a production service-level promise. Throughput depends
			on ledger mix, storage, database size, and the derived facts enabled for a run.
		</p>
	</section>

	<section class="doc-section" id="retention">
		<div class="doc-section-heading">
			<p class="doc-kicker">04 / Compact retention</p>
			<h2>Store the dependency graph, keep raw detail in the archive.</h2>
		</div>
		<p>A transaction enters the compact database when it contains at least one of:</p>
		<ul class="doc-check-list">
			<li>a direct or Soroban-authorized contract invocation;</li>
			<li>a contract event or diagnostic-event touch;</li>
			<li>a contract executable observation such as creation, update, restoration, or removal;</li>
			<li>a contract Wasm code observation.</li>
		</ul>
		<p>
			Ordinary transactions without contract evidence are omitted. Failed transactions remain when they
			contain useful contract evidence, but failed state changes are never materialized as current state.
			The canonical transaction hash, ledger sequence, apply order, and archive path provide the durable
			route back to full local evidence.
		</p>
	</section>

	<section class="doc-section" id="resume">
		<div class="doc-section-heading">
			<p class="doc-kicker">05 / Continuity and resume</p>
			<h2>The checkpoint is a committed contiguous range.</h2>
		</div>
		<ol class="doc-numbered-list">
			<li>
				<strong>Bind the database to one network.</strong>
				<span>A network passphrase mismatch stops the run.</span>
			</li>
			<li>
				<strong>Verify the ledger chain.</strong>
				<span>Adjacent ledger hashes must agree before state advances.</span>
			</li>
			<li>
				<strong>Reject gaps and prepends.</strong>
				<span>Schema v2 may resume or append the next ledger; it cannot silently fill a sparse range.</span>
			</li>
			<li>
				<strong>Commit the whole batch.</strong>
				<span>A decode or write failure rolls back the active SQLite transaction.</span>
			</li>
			<li>
				<strong>Resume from durable state.</strong>
				<span>Rerunning skips committed ledgers and retries only uncommitted work.</span>
			</li>
		</ol>
	</section>

	<section class="doc-section" id="finalizer">
		<div class="doc-section-heading">
			<p class="doc-kicker">06 / Serving projection</p>
			<h2>Read performance is built after historical ingestion.</h2>
		</div>
		<p>
			The evidence database is ordered for append-efficient ingestion. The wallet finalizer opens it
			strictly read-only, streams exact transaction-wallet edges into disk shards, aggregates one shard
			at a time, adds contract and verified-protocol counts, creates both ranking indexes, validates the
			result, and atomically renames the completed serving database into place.
		</p>
		<StatusNote status="Implemented" title="Safe publication">
			The API validates that the serving snapshot covers the exact same ledger range as the compact
			database. If coverage moves ahead, rankings remain unavailable until a matching snapshot is
			published; stale values are not served as current.
		</StatusNote>
	</section>
</article>
