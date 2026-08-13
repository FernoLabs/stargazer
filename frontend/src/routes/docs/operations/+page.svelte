<script lang="ts">
	import StatusNote from '$lib/components/docs/StatusNote.svelte';
</script>

<svelte:head>
	<title>Operations and recovery · Stargazer documentation</title>
	<meta
		name="description"
		content="Operational invariants, health signals, recovery procedures, database publication, and failure modes for Stargazer's archive and future live indexers."
	/>
	<link rel="canonical" href="https://stargazer.ferno.ag/docs/operations" />
</svelte:head>

<article class="doc-page">
	<header class="doc-hero">
		<p class="doc-kicker">Reference / Operations</p>
		<h1>Recover from durable evidence, not from assumptions.</h1>
		<p class="doc-lede">
			Stargazer treats the raw archive and committed ledger checkpoint as the recovery boundary. Workers
			may restart, databases may be rebuilt, and serving projections may be replaced, but the system
			must never skip history or serve a projection whose coverage no longer matches its evidence store.
		</p>
	</header>

	<section class="doc-section" id="invariants">
		<div class="doc-section-heading">
			<p class="doc-kicker">01 / Invariants</p>
			<h2>Conditions that stop the pipeline.</h2>
		</div>
		<ul class="doc-check-list">
			<li>the database is permanently bound to one Stellar network ID;</li>
			<li>committed ledgers form one contiguous range and agree on the hash chain;</li>
			<li>parallel decoders may finish out of order, but commits remain in ledger order;</li>
			<li>a failed decode or write cannot advance the committed checkpoint;</li>
			<li>failed transactions never mutate materialized current state;</li>
			<li>historical replay never enters the live notification outbox;</li>
			<li>an unrecognized parser or schema version fails visibly instead of being guessed;</li>
			<li>the read API rejects a serving projection with stale or incompatible coverage.</li>
		</ul>
	</section>

	<section class="doc-section" id="health">
		<div class="doc-section-heading">
			<p class="doc-kicker">02 / Health model</p>
			<h2>Expose progress at every boundary.</h2>
		</div>
		<div class="table-scroll">
			<table>
				<thead>
					<tr>
						<th>Signal</th>
						<th>Why it matters</th>
						<th>Status</th>
					</tr>
				</thead>
				<tbody>
					<tr>
						<td>Archive partition freshness and completion</td>
						<td>Shows whether the local recovery surface is current and complete.</td>
						<td>Partially implemented in markers and logs</td>
					</tr>
					<tr>
						<td>Indexed ledger start/end and contract transaction count</td>
						<td>States the exact bounded coverage served by the demo.</td>
						<td>Implemented in <code>/health</code></td>
					</tr>
					<tr>
						<td>Last fetched, decoded, and committed ledger</td>
						<td>Separates source lag, parser lag, and database lag.</td>
						<td>Planned for live ingestion</td>
					</tr>
					<tr>
						<td>Parser failures by module and version</td>
						<td>Prevents silent coverage loss after a protocol upgrade.</td>
						<td>Planned</td>
					</tr>
					<tr>
						<td>Serving generation and coverage match</td>
						<td>Stops stale rankings when the compact range advances.</td>
						<td>Implemented</td>
					</tr>
					<tr>
						<td>Outbox age, retries, and channel delivery status</td>
						<td>Separates detection success from notification success.</td>
						<td>Planned</td>
					</tr>
				</tbody>
			</table>
		</div>
	</section>

	<section class="doc-section" id="failure-modes">
		<div class="doc-section-heading">
			<p class="doc-kicker">03 / Failure modes</p>
			<h2>Expected response by failure class.</h2>
		</div>
		<div class="incident-list">
			<section>
				<h3>Remote source unavailable</h3>
				<p>Keep the committed checkpoint, retry with bounded backoff, then select the documented fallback source.</p>
			</section>
			<section>
				<h3>Archive ledger missing</h3>
				<p>Stop before opening a commit that would create a sparse range. Resynchronize that partition and retry.</p>
			</section>
			<section>
				<h3>Hash conflict</h3>
				<p>Stop ingestion and raise an operator incident. Do not resolve conflicting history automatically.</p>
			</section>
			<section>
				<h3>Unsupported XDR or Wasm</h3>
				<p>Preserve the raw record and coverage failure; generic facts may continue only when the ledger itself remains safely decodable.</p>
			</section>
			<section>
				<h3>Serving snapshot stale</h3>
				<p>Keep exact compact relationships available, return rankings as unavailable, rebuild, validate, and atomically publish.</p>
			</section>
			<section>
				<h3>Delivery channel unavailable</h3>
				<p>Retain the committed outbox event, retry idempotently, and report channel failure without rolling back chain facts.</p>
			</section>
		</div>
	</section>

	<section class="doc-section" id="publication">
		<div class="doc-section-heading">
			<p class="doc-kicker">04 / Publication</p>
			<h2>Build beside production, then rename.</h2>
		</div>
		<ol class="doc-numbered-list">
			<li>
				<strong>Open historical evidence read-only.</strong>
				<span>The finalizer cannot modify the compact database.</span>
			</li>
			<li>
				<strong>Write a new generation.</strong>
				<span>Progress checkpoints and shard files make a long aggregation resumable.</span>
			</li>
			<li>
				<strong>Create query indexes last.</strong>
				<span>Bulk aggregation stays sequential and avoids maintaining rankings row by row.</span>
			</li>
			<li>
				<strong>Validate schema and coverage.</strong>
				<span>Required metadata and exact index key ordering are part of the read contract.</span>
			</li>
			<li>
				<strong>Atomically replace the active file.</strong>
				<span>The read API detects the new generation on the next request without restarting.</span>
			</li>
		</ol>
	</section>

	<section class="doc-section" id="backup">
		<div class="doc-section-heading">
			<p class="doc-kicker">05 / Backup priority</p>
			<h2>Back up what cannot be regenerated first.</h2>
		</div>
		<div class="table-scroll">
			<table>
				<thead>
					<tr>
						<th>Asset</th>
						<th>Regenerable?</th>
						<th>Priority</th>
					</tr>
				</thead>
				<tbody>
					<tr>
						<td>Protocol registry sources, parser versions, and configuration</td>
						<td>No—not without reconstructing research decisions</td>
						<td>Highest</td>
					</tr>
					<tr>
						<td>Alert-control policy and future delivery state</td>
						<td>No</td>
						<td>Highest</td>
					</tr>
					<tr>
						<td>Local raw archive</td>
						<td>Usually, while the upstream object remains available</td>
						<td>High due to recovery time and bandwidth</td>
					</tr>
					<tr>
						<td>Compact evidence database</td>
						<td>Yes, from raw archive plus exact code/config version</td>
						<td>Medium</td>
					</tr>
					<tr>
						<td>Serving snapshot</td>
						<td>Yes, from the compact database</td>
						<td>Low</td>
					</tr>
				</tbody>
			</table>
		</div>
		<StatusNote status="Planned" title="Recovery proof">
			A production milestone is not complete until a clean-environment restore has rebuilt the read path,
			validated its coverage, and demonstrated that source failover and gap recovery work as documented.
		</StatusNote>
	</section>
</article>
