<script lang="ts">
	import StatusNote from '$lib/components/docs/StatusNote.svelte';
</script>

<svelte:head>
	<title>Data and trust model · Stargazer documentation</title>
	<meta
		name="description"
		content="Reference for Stargazer raw evidence, compact and serving databases, exact wallet-contract relationships, protocol attribution, executable history, and alert provenance."
	/>
	<link rel="canonical" href="https://stargazer.ferno.ag/docs/data-model" />
</svelte:head>

<article class="doc-page">
	<header class="doc-hero">
		<p class="doc-kicker">Reference / Data model</p>
		<h1>Separate chain facts, attribution, projections, and delivery state.</h1>
		<p class="doc-lede">
			Stargazer stores the smallest durable facts required to prove a relationship or change, while the
			raw archive preserves everything needed for later decoding. Human labels, protocol semantics,
			wallet rankings, and notification state live in separate layers with different trust boundaries.
		</p>
	</header>

	<section class="doc-section" id="stores">
		<div class="doc-section-heading">
			<p class="doc-kicker">01 / Storage layers</p>
			<h2>Four stores, four purposes.</h2>
		</div>
		<div class="storage-stack">
			<section>
				<span>Layer 1 · Evidence</span>
				<h3>Raw Galexie archive</h3>
				<p>Immutable compressed ledger objects. The durable fallback when a new field must be parsed.</p>
			</section>
			<section>
				<span>Layer 2 · Historical facts</span>
				<h3>Compact SQLite database</h3>
				<p>Ordered ledgers, retained contract transactions, graph edges, attribution roots, and executable history.</p>
			</section>
			<section>
				<span>Layer 3 · Read projection</span>
				<h3>Serving SQLite database</h3>
				<p>Wallet activity totals and ranking indexes rebuilt from exact historical edges.</p>
			</section>
			<section>
				<span>Layer 4 · User control</span>
				<h3>Alert-control database</h3>
				<p>Telegram pairing, alert policy, and entitlement state. It never rewrites ledger evidence.</p>
			</section>
		</div>
	</section>

	<section class="doc-section" id="schema">
		<div class="doc-section-heading">
			<p class="doc-kicker">02 / Compact schema</p>
			<h2>Append-efficient facts and small indexed histories.</h2>
		</div>
		<div class="schema-groups">
			<section>
				<h3>Checkpoint and provenance</h3>
				<code>compact_meta</code>
				<code>compact_ledgers</code>
				<p>Network binding, ledger hash chain, close time, archive path, and retained counts.</p>
			</section>
			<section>
				<h3>Transaction graph</h3>
				<code>contract_transactions</code>
				<code>transaction_wallets</code>
				<code>transaction_contracts</code>
				<code>transaction_wallet_contracts</code>
				<code>wallet_contracts</code>
			</section>
			<section>
				<h3>Attribution and usage</h3>
				<code>protocols</code>
				<code>protocol_contracts</code>
				<code>transaction_protocols</code>
				<code>protocol_usage</code>
				<code>contract_usage</code>
			</section>
			<section>
				<h3>Contract lifecycle</h3>
				<code>contract_executable_history</code>
				<code>contract_current_executable</code>
				<code>contract_availability_events</code>
				<code>contract_code</code>
			</section>
		</div>
		<p>
			Large account-first and ranking indexes are intentionally omitted during bulk ingestion. The
			post-backfill finalizer builds them in a separate serving generation, avoiding random index
			maintenance on the historical hot path.
		</p>
	</section>

	<section class="doc-section" id="wallet-contract">
		<div class="doc-section-heading">
			<p class="doc-kicker">03 / Exact relationships</p>
			<h2>A transaction participant is not automatically a protocol user.</h2>
		</div>
		<p>Stargazer creates a wallet-to-contract edge only from one of two evidence paths:</p>
		<ol class="doc-numbered-list">
			<li>
				<strong>Effective operation source.</strong>
				<span>A direct contract call links to that operation's explicit source, or to the transaction source when no operation source exists.</span>
			</li>
			<li>
				<strong>Soroban authorization subtree.</strong>
				<span>An authorized account links only to contracts actually present inside its authorization subtree.</span>
			</li>
		</ol>
		<p>
			Fee-only participants, event-only touches, diagnostic-only touches, and executable-version
			observations remain valid transaction evidence but do not invent a wallet dependency. Muxed
			accounts normalize to their base <code>G...</code> identity while preserving the distinction in raw
			evidence.
		</p>
		<StatusNote status="Implemented" title="Why this matters">
			The former shortcut of pairing every wallet in a transaction with every contract would overstate
			coverage. The compact index now stores exact per-transaction wallet-contract edges before building
			the deduplicated relationship table.
		</StatusNote>
	</section>

	<section class="doc-section" id="transactions">
		<div class="doc-section-heading">
			<p class="doc-kicker">04 / Transaction identity</p>
			<h2>Store the hash needed for retrieval, not a misleading “signature.”</h2>
		</div>
		<p>
			The durable identifier is Stellar's canonical 32-byte transaction hash. A transaction envelope can
			contain several decorated signatures, so “transaction signature” is not a unique retrieval key.
			The compact database stores the hash, ledger sequence, apply order, success flag, and archive path.
		</p>
		<p>
			Old RPC calls may fall outside provider retention. The local archive coordinates are therefore the
			authoritative retrieval path for historical transaction evidence.
		</p>
	</section>

	<section class="doc-section" id="attribution">
		<div class="doc-section-heading">
			<p class="doc-kicker">05 / Protocol attribution</p>
			<h2>Labels require a source.</h2>
		</div>
		<div class="architecture-lanes">
			<section>
				<h3>Attributed</h3>
				<p>
					A contract exactly matches a source-backed registry root with a protocol name, role, and optional
					source URL. A protocol is counted at most once per transaction.
				</p>
			</section>
			<section>
				<h3>Unattributed</h3>
				<p>
					The contract remains queryable using its checksum-valid <code>C...</code> address. Stargazer does
					not hide it or invent a project identity.
				</p>
			</section>
		</div>
		<p>
			Factory-child expansion, ownership inference, and time-valid registry mappings remain roadmap
			work. A materially changed registry currently requires a fresh compact database to avoid stale
			mappings.
		</p>
	</section>

	<section class="doc-section" id="versions">
		<div class="doc-section-heading">
			<p class="doc-kicker">06 / Contract versions</p>
			<h2>An upload is not an upgrade.</h2>
		</div>
		<div class="table-scroll">
			<table>
				<thead>
					<tr>
						<th>Observation</th>
						<th>Stored meaning</th>
						<th>Alert meaning</th>
					</tr>
				</thead>
				<tbody>
					<tr>
						<td>Wasm upload</td>
						<td>Code hash and first observed provenance</td>
						<td>Precursor only; no instance changed</td>
					</tr>
					<tr>
						<td>First state in a bounded window</td>
						<td>Non-exact baseline, active before the window</td>
						<td>Not presented as a deployment or upgrade</td>
					</tr>
					<tr>
						<td>Created or changed executable</td>
						<td>Chronological instance executable transition</td>
						<td>Eligible upgrade evidence</td>
					</tr>
					<tr>
						<td>Restoration with same executable</td>
						<td>Availability event only</td>
						<td>Not an upgrade</td>
					</tr>
					<tr>
						<td>Removal</td>
						<td>Availability event; history is preserved</td>
						<td>Lifecycle signal</td>
					</tr>
				</tbody>
			</table>
		</div>
		<p>
			The current implementation treats the contract-instance ledger diff as canonical. Corroborating
			that transition with the system <code>executable_update</code> event remains a later normalization
			step.
		</p>
	</section>

	<section class="doc-section" id="alerts">
		<div class="doc-section-heading">
			<p class="doc-kicker">07 / Alert provenance</p>
			<h2>Every alert should say how it was produced.</h2>
		</div>
		<dl class="definition-grid">
			<div>
				<dt>Archive-derived transition</dt>
				<dd>Deterministic executable history reconstructed from retained ledgers.</dd>
			</div>
			<div>
				<dt>Historical incident finding</dt>
				<dd>A bounded, versioned correlation rule with replay coverage and delivery disabled.</dd>
			</div>
			<div>
				<dt>Forward controller alert</dt>
				<dd>A post-execution Classic account policy or activity change from the governance watcher.</dd>
			</div>
			<div>
				<dt>Future live finding</dt>
				<dd>A committed live-ledger rule result eligible for an outbox and user delivery policy.</dd>
			</div>
		</dl>
		<p>
			AI-generated explanation is never the evidence boundary. It may summarize a deterministic finding,
			but the underlying coordinates, parser version, primary sources, and coverage limits remain the
			verifiable record.
		</p>
	</section>
</article>
