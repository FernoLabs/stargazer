<script lang="ts">
	import PipelineFlow from '$lib/components/docs/PipelineFlow.svelte';
	import StatusNote from '$lib/components/docs/StatusNote.svelte';

	const moduleLifecycle = [
		{
			label: 'Define',
			detail: 'Version the module, its accepted facts, output schema, and supported code hashes.',
			status: 'Planned'
		},
		{
			label: 'Fixture',
			detail: 'Capture representative real ledgers and exact expected outputs.',
			status: 'In progress'
		},
		{
			label: 'Backfill',
			detail: 'Replay the local archive into isolated staging storage with delivery disabled.',
			status: 'Planned'
		},
		{
			label: 'Validate',
			detail: 'Check continuity, idempotency, parser coverage, errors, and output invariants.',
			status: 'Planned'
		},
		{
			label: 'Catch up',
			detail: 'Process the gap between the archive high-water mark and the current live checkpoint.',
			status: 'Planned'
		},
		{
			label: 'Publish',
			detail: 'Atomically promote the materialization and enable the same version on the live tail.',
			status: 'Planned'
		}
	] as const;
</script>

<svelte:head>
	<title>Adding and backfilling an indexer · Stargazer documentation</title>
	<meta
		name="description"
		content="The Stargazer workflow for developing a new versioned indexer module, replaying it against the local archive, validating it, and handing it to live ingestion."
	/>
	<link rel="canonical" href="https://stargazer.ferno.ag/docs/backfill" />
</svelte:head>

<article class="doc-page">
	<header class="doc-hero">
		<p class="doc-kicker">Workflow / New indexer</p>
		<h1>Build offline. Prove the backfill. Then join live.</h1>
		<p class="doc-lede">
			A new protocol decoder, security rule, or derived index should not begin at today's ledger and
			ignore the history already retained. Stargazer's target workflow replays every new module against
			the local archive, validates the result in isolation, catches it up to the live watermark, and only
			then promotes it.
		</p>
	</header>

	<StatusNote status="In progress" title="Target module workflow">
		The archive can already be replayed deterministically and the serving database is published
		atomically. The compact writer itself is still monolithic, so extracting a stable pluggable module
		interface is required before this workflow is fully automated.
	</StatusNote>

	<section class="doc-section" id="lifecycle">
		<div class="doc-section-heading">
			<p class="doc-kicker">01 / Lifecycle</p>
			<h2>One path from first fixture to live operation.</h2>
		</div>
		<PipelineFlow steps={moduleLifecycle} />
	</section>

	<section class="doc-section" id="contract">
		<div class="doc-section-heading">
			<p class="doc-kicker">02 / Module contract</p>
			<h2>Version behavior, not just code.</h2>
		</div>
		<p>Each indexing module should declare enough information to reproduce a finding later:</p>
		<div class="table-scroll">
			<table>
				<thead>
					<tr>
						<th>Field</th>
						<th>Purpose</th>
					</tr>
				</thead>
				<tbody>
					<tr>
						<td><code>module_id</code></td>
						<td>Stable identity for a protocol decoder, generic fact extractor, or rule.</td>
					</tr>
					<tr>
						<td><code>module_version</code></td>
						<td>Immutable behavior version attached to every derived row.</td>
					</tr>
					<tr>
						<td><code>schema_version</code></td>
						<td>Expected storage contract and migration boundary.</td>
					</tr>
					<tr>
						<td><code>recognized_wasm</code></td>
						<td>Exact code hashes a protocol-specific decoder understands.</td>
					</tr>
					<tr>
						<td><code>input_facts</code></td>
						<td>Normalized chain facts required before the module can run.</td>
					</tr>
					<tr>
						<td><code>coverage</code></td>
						<td>Network, ledger range, contracts, and known exclusions for the materialization.</td>
					</tr>
				</tbody>
			</table>
		</div>
		<p>
			An unknown Wasm hash must continue through generic indexing and create a coverage finding. It must
			not be decoded silently with the closest known interface.
		</p>
	</section>

	<section class="doc-section" id="fixtures">
		<div class="doc-section-heading">
			<p class="doc-kicker">03 / Fixtures and benchmarks</p>
			<h2>Start with difficult, real ledgers.</h2>
		</div>
		<ol class="doc-numbered-list">
			<li>
				<strong>Choose representative history.</strong>
				<span>Include high-transaction ledgers, failed invocations, fee bumps, deep authorization trees, upgrades, and restorations.</span>
			</li>
			<li>
				<strong>Pin the raw evidence.</strong>
				<span>Record ledger hashes, archive paths, transaction hashes, and checksums outside generated databases.</span>
			</li>
			<li>
				<strong>Assert semantic outputs.</strong>
				<span>Test exact wallet-contract links, protocol attribution, executable transitions, and known negative cases.</span>
			</li>
			<li>
				<strong>Measure the cost.</strong>
				<span>Compare ledgers per second, decoded bytes, database growth, write amplification, and peak memory.</span>
			</li>
			<li>
				<strong>Replay twice.</strong>
				<span>The second pass must leave counts and materialized state unchanged.</span>
			</li>
		</ol>
	</section>

	<section class="doc-section" id="staging">
		<div class="doc-section-heading">
			<p class="doc-kicker">04 / Isolated backfill</p>
			<h2>Never mutate the active projection in place.</h2>
		</div>
		<div class="handoff-diagram handoff-diagram-wide" aria-label="Backfill and live handoff">
			<div>
				<span>Active generation</span>
				<strong>Module v1 + live watermark</strong>
			</div>
			<i aria-hidden="true">∥</i>
			<div>
				<span>Staging generation</span>
				<strong>Module v2 + archive replay</strong>
			</div>
			<i aria-hidden="true">→</i>
			<div>
				<span>Catch-up buffer</span>
				<strong>Archive high-water → live</strong>
			</div>
			<i aria-hidden="true">→</i>
			<div>
				<span>Atomic promotion</span>
				<strong>v2 becomes active</strong>
			</div>
		</div>
		<p>
			Staging can be a new database, a versioned table set, or a separate storage generation. The key
			property is that readers see either the complete old generation or the complete validated new
			generation—never a partially rebuilt mixture.
		</p>
	</section>

	<section class="doc-section" id="validation">
		<div class="doc-section-heading">
			<p class="doc-kicker">05 / Promotion gate</p>
			<h2>What must pass before live activation.</h2>
		</div>
		<ul class="doc-check-list">
			<li>the full target ledger range is contiguous and hash-chain consistent;</li>
			<li>all fixtures match their exact expected facts and findings;</li>
			<li>duplicate replay changes no counts, versions, or alert identities;</li>
			<li>parser errors and unsupported code hashes are counted and reviewable;</li>
			<li>archive and live source outputs agree over an overlapping sample;</li>
			<li>the new projection covers the live high-water mark or a documented handoff ledger;</li>
			<li>read queries and indexes meet their bounded latency and memory budgets;</li>
			<li>historical delivery remains disabled throughout replay.</li>
		</ul>
	</section>

	<section class="doc-section" id="delivery">
		<div class="doc-section-heading">
			<p class="doc-kicker">06 / Alert semantics</p>
			<h2>Historical detection is not a new incident.</h2>
		</div>
		<p>
			A backfill may discover an old upgrade, control change, or incident exposure. That finding belongs
			in the evidence database and can appear in a retrospective report, but it must carry historical
			provenance and <code>delivery_enabled = false</code>. Only findings produced after the promoted live
			handoff are eligible for normal notification policy.
		</p>
		<StatusNote status="Planned" title="Operator-controlled replay">
			If Stargazer later supports intentional retrospective campaigns, they should use a separate,
			audited operator action rather than weakening the default silent-backfill invariant.
		</StatusNote>
	</section>
</article>
