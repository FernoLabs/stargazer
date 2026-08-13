<script lang="ts">
	import PipelineFlow from '$lib/components/docs/PipelineFlow.svelte';
	import StatusNote from '$lib/components/docs/StatusNote.svelte';

	const livePipeline = [
		{
			label: 'Acquire',
			detail: 'Follow newly closed ledgers from RPC first and Captive Core when self-operation is justified.',
			status: 'Planned'
		},
		{
			label: 'Normalize',
			detail: 'Convert every source into the same versioned ledger input used by indexing modules.',
			status: 'In progress'
		},
		{
			label: 'Verify',
			detail: 'Require the next sequence and previous-ledger hash to match the committed watermark.',
			status: 'Planned'
		},
		{
			label: 'Index',
			detail: 'Run generic facts, protocol modules, and deterministic rules in ledger order.',
			status: 'Planned'
		},
		{
			label: 'Commit',
			detail: 'Persist facts, findings, and checkpoint in one atomic transaction.',
			status: 'Planned'
		},
		{
			label: 'Deliver',
			detail: 'Publish only committed live findings through a durable outbox.',
			status: 'Planned'
		}
	] as const;
</script>

<svelte:head>
	<title>Live ingestion · Stargazer documentation</title>
	<meta
		name="description"
		content="Target design for Stargazer live Stellar ledger ingestion, ordered checkpoints, gap recovery, shared indexers, and post-commit alerts."
	/>
	<link rel="canonical" href="https://stargazer.ferno.ag/docs/live-ingestion" />
</svelte:head>

<article class="doc-page">
	<header class="doc-hero">
		<p class="doc-kicker">Pipeline / Live ingestion</p>
		<h1>Live is a new source, not a second parser.</h1>
		<p class="doc-lede">
			The live service will acquire recently closed Stellar ledgers, verify continuity, and submit them
			to the same versioned processing boundary used for archive replay. A gap switches the system into
			catch-up mode; it does not permit the live checkpoint to jump forward.
		</p>
	</header>

	<StatusNote status="Planned" title="Not yet a continuous ledger service">
		The repository contains the source strategy and the optimized historical indexer, but continuous
		<code>LedgerCloseMeta</code> ingestion is not wired today. The forward governance watcher is a
		separate implemented service and should not be confused with a complete live ledger tail.
	</StatusNote>

	<section class="doc-section" id="pipeline">
		<div class="doc-section-heading">
			<p class="doc-kicker">01 / Target data path</p>
			<h2>From ledger close to committed finding.</h2>
		</div>
		<PipelineFlow steps={livePipeline} />
	</section>

	<section class="doc-section" id="sources">
		<div class="doc-section-heading">
			<p class="doc-kicker">02 / Source policy</p>
			<h2>Choose the source by recovery need.</h2>
		</div>
		<div class="table-scroll">
			<table>
				<thead>
					<tr>
						<th>Need</th>
						<th>Preferred source</th>
						<th>Fallback</th>
					</tr>
				</thead>
				<tbody>
					<tr>
						<td>Development and first live tail</td>
						<td>RPC <code>getLedgers</code></td>
						<td>Captured fixtures</td>
					</tr>
					<tr>
						<td>Production live tail</td>
						<td>Captive Core</td>
						<td>RPC ledger backend</td>
					</tr>
					<tr>
						<td>Short gap recovery</td>
						<td>RPC ledger backend</td>
						<td>Archive RPC <code>getLedgers</code></td>
					</tr>
					<tr>
						<td>Long gap or new module backfill</td>
						<td>Local Galexie mirror</td>
						<td>Archive RPC</td>
					</tr>
				</tbody>
			</table>
		</div>
		<p>
			RPC retention is finite. The live service must record each source's oldest and newest available
			ledger and switch to archive recovery before an outage moves outside the recoverable window.
		</p>
	</section>

	<section class="doc-section" id="checkpoint">
		<div class="doc-section-heading">
			<p class="doc-kicker">03 / Commit protocol</p>
			<h2>Acknowledgement follows durable state.</h2>
		</div>
		<ol class="doc-numbered-list">
			<li>
				<strong>Fetch the next ledger.</strong>
				<span>Record source type, endpoint, available range, and raw evidence reference.</span>
			</li>
			<li>
				<strong>Verify the chain coordinate.</strong>
				<span>The sequence must be checkpoint + 1 and the previous hash must match.</span>
			</li>
			<li>
				<strong>Run every enabled module.</strong>
				<span>Generic facts and versioned protocol decoders consume the same normalized input.</span>
			</li>
			<li>
				<strong>Commit facts and checkpoint together.</strong>
				<span>A parser or database failure leaves the previous checkpoint authoritative.</span>
			</li>
			<li>
				<strong>Publish findings after commit.</strong>
				<span>Delivery consumes a durable outbox; it cannot make an uncommitted alert visible.</span>
			</li>
		</ol>
	</section>

	<section class="doc-section" id="gaps">
		<div class="doc-section-heading">
			<p class="doc-kicker">04 / Gap recovery</p>
			<h2>Pause, fill, reconcile, resume.</h2>
		</div>
		<div class="handoff-diagram" aria-label="Live gap recovery sequence">
			<div>
				<span>Healthy tail</span>
				<strong>Ledger N committed</strong>
			</div>
			<i aria-hidden="true">→</i>
			<div>
				<span>Gap detected</span>
				<strong>Do not advance</strong>
			</div>
			<i aria-hidden="true">→</i>
			<div>
				<span>Archive catch-up</span>
				<strong>Replay N + 1 … M</strong>
			</div>
			<i aria-hidden="true">→</i>
			<div>
				<span>Reconciled</span>
				<strong>Resume at M + 1</strong>
			</div>
		</div>
		<p>
			If a source returns a conflicting hash for an already committed ledger, ingestion stops and raises
			an operator incident. Automatic recovery is appropriate for missing data, not for conflicting
			history.
		</p>
	</section>

	<section class="doc-section" id="alerts">
		<div class="doc-section-heading">
			<p class="doc-kicker">05 / Alert boundary</p>
			<h2>Detection and delivery are different guarantees.</h2>
		</div>
		<div class="architecture-lanes">
			<section>
				<h3>Implemented today</h3>
				<ul>
					<li>Archive-derived executable transition records</li>
					<li>A narrow historical YieldBlox correlation rule</li>
					<li>Executed Classic controller policy and activity alerts</li>
					<li>Telegram identity pairing and user policy storage</li>
				</ul>
			</section>
			<section>
				<h3>Still required</h3>
				<ul>
					<li>Durable event outbox after the live database commit</li>
					<li>Idempotent Telegram delivery worker and retry state</li>
					<li>Per-user alert history and channel delivery evidence</li>
					<li>Payment settlement only after delivery is dependable</li>
				</ul>
			</section>
		</div>
	</section>

	<section class="doc-section" id="multisig">
		<div class="doc-section-heading">
			<p class="doc-kicker">06 / Multisig scope</p>
			<h2>On-chain execution is visible; private proposals are not.</h2>
		</div>
		<p>
			The current governance watcher can detect changes to Classic signer sets and thresholds, and it
			can retain the bounded payload of successful operations sourced by tracked controller accounts.
			It cannot observe an unsigned or partially signed envelope being exchanged off-chain, nor can it
			infer which named signers approved an executed transaction.
		</p>
		<StatusNote status="Planned" title="Pending multisig payloads">
			Pre-execution monitoring requires a project-specific multisig coordinator, repository, or explicit
			envelope submission channel. Stargazer will add those sources only where a verifiable integration
			exists.
		</StatusNote>
	</section>
</article>
