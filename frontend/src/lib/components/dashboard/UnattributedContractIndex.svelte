<script lang="ts">
	import { resolve } from '$app/paths';
	import ContractIdenticon from '$lib/components/dashboard/ContractIdenticon.svelte';
	import { formatAddress, formatExactCount } from '$lib/dashboard/format';
	import type {
		LiveUnattributedContractIndexResponse,
		StellarContractAddress
	} from '$lib/dashboard';

	interface Props {
		index: LiveUnattributedContractIndexResponse;
		cursor: StellarContractAddress | null;
		filter: 'all' | 'verified' | 'unattributed';
	}

	let { index, cursor, filter }: Props = $props();

	const dateFormatter = new Intl.DateTimeFormat('en-GB', {
		day: '2-digit',
		month: 'short',
		year: 'numeric',
		hour: '2-digit',
		minute: '2-digit',
		hourCycle: 'h23',
		timeZone: 'UTC'
	});

	function formatDate(value: string | null): string {
		return value ? `${dateFormatter.format(new Date(value))} UTC` : 'Date unavailable';
	}

	function shortHash(value: string | null): string {
		return value ? `${value.slice(0, 10)}…${value.slice(-8)}` : 'None';
	}

	function explorerUrl(address: string): string {
		return `https://stellar.expert/explorer/public/contract/${address}`;
	}

	function pageHref(nextCursor?: string): string {
		const parameters = new URLSearchParams();
		if (filter !== 'all') parameters.set('attribution', filter);
		if (nextCursor) parameters.set('unattributedCursor', nextCursor);
		const query = parameters.toString();
		const path = resolve('/app/protocols');
		return `${path}${query ? `?${query}` : ''}#unattributed-title`;
	}
</script>

<section class="unattributed-index" aria-labelledby="unattributed-title">
	<header class="section-heading">
		<div>
			<h2 id="unattributed-title">Unattributed protocol contracts</h2>
			<p>
				{formatExactCount(index.items.length)} contracts on this page, ordered by on-chain
				C-address.
			</p>
		</div>
		<p>
			A contract may be a standalone protocol or a child of one. Until verified, its C-address is
			the placeholder identity; address order is not an activity ranking.
		</p>
	</header>

	{#if index.items.length === 0}
		<p class="empty-state">No unattributed contract was returned on this page.</p>
	{:else}
		<div class="table-scroll">
			<table>
				<thead>
					<tr>
						<th scope="col">On-chain placeholder</th>
						<th scope="col">First observed</th>
						<th scope="col">Last observed</th>
						<th scope="col">Network transactions</th>
						<th scope="col">Current executable</th>
					</tr>
				</thead>
				<tbody>
					{#each index.items as contract (contract.contractAddress)}
						<tr>
							<th scope="row">
								<div class="contract-identity">
									<ContractIdenticon contractAddress={contract.placeholderProtocolId} size="small" />
									<div>
										<a
											href={explorerUrl(contract.contractAddress)}
											target="_blank"
											rel="noreferrer"
											title={contract.contractAddress}
										>
											{formatAddress(contract.placeholderProtocolId, { start: 10, end: 8 })} ↗
										</a>
										<span>Protocol attribution pending</span>
									</div>
								</div>
							</th>
							<td>
								<time datetime={contract.firstSeenAt ?? undefined}>{formatDate(contract.firstSeenAt)}</time>
								<span>ledger {formatExactCount(contract.firstLedger)}</span>
							</td>
							<td>
								<time datetime={contract.lastSeenAt ?? undefined}>{formatDate(contract.lastSeenAt)}</time>
								<span>ledger {formatExactCount(contract.lastLedger)}</span>
							</td>
							<td>
								<strong>{formatExactCount(contract.transactionCount)}</strong>
								<span>{formatExactCount(contract.successfulTransactionCount)} successful</span>
							</td>
							<td>
								{#if contract.currentExecutable}
									<strong>{contract.currentExecutable.executableKind}</strong>
									<code title={contract.currentExecutable.wasmHash ?? undefined}>
										{shortHash(contract.currentExecutable.wasmHash)}
									</code>
									<span>ledger {formatExactCount(contract.currentExecutable.ledger)}</span>
								{:else}
									<span>No executable observation</span>
								{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}

	{#if index.nextCursor}
		<nav class="index-pagination" aria-label="Unattributed contract index pages">
			<p>More contracts are available in contract-ID order.</p>
			<div>
				{#if cursor}
					<a href={pageHref()}>← First page</a>
				{/if}
				<a href={pageHref(index.nextCursor)}>Next contracts →</a>
			</div>
		</nav>
	{:else if cursor}
		<nav class="index-pagination" aria-label="Unattributed contract index pages">
			<p>End of the unattributed contract index.</p>
			<a href={pageHref()}>← First page</a>
		</nav>
	{/if}
</section>

<style>
	.unattributed-index {
		padding-top: 1.5rem;
	}

	.section-heading {
		display: flex;
		justify-content: space-between;
		gap: 3rem;
		align-items: end;
		margin-bottom: 1rem;
	}

	h2,
	p {
		margin-top: 0;
	}

	h2 {
		margin-bottom: 0.3rem;
		font: 550 1.55rem/1.2 'Archivo SemiCondensed', sans-serif;
	}

	.section-heading p {
		max-width: 42rem;
		margin-bottom: 0;
		color: var(--app-muted);
		font-size: 0.82rem;
		line-height: 1.45;
	}

	.table-scroll {
		overflow-x: auto;
		border-top: 2px solid var(--app-ink);
		background: var(--app-paper);
	}

	table {
		width: 100%;
		min-width: 940px;
		border-collapse: collapse;
		font-size: 0.76rem;
		text-align: left;
	}

	th,
	td {
		padding: 0.8rem;
		border-bottom: 1px solid var(--app-rule);
		vertical-align: top;
	}

	thead th {
		color: var(--app-muted);
		font-size: 0.66rem;
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}

	td span,
	th span,
	td code {
		display: block;
		margin-top: 0.2rem;
		color: var(--app-muted);
		font-size: 0.66rem;
	}

	code,
	time {
		font-family: 'Fragment Mono', monospace;
	}

	a {
		color: var(--app-violet);
		font-weight: 600;
		text-decoration: none;
	}

	a:hover {
		text-decoration: underline;
		text-underline-offset: 0.2em;
	}

	.contract-identity {
		display: flex;
		gap: 0.55rem;
		align-items: center;
	}

	.contract-identity > div {
		min-width: 0;
	}

	.empty-state {
		padding: 1rem;
		border-top: 1px solid var(--app-ink);
		border-bottom: 1px solid var(--app-rule);
		background: var(--app-paper);
	}

	.index-pagination {
		display: flex;
		justify-content: space-between;
		gap: 1rem;
		align-items: center;
		padding: 0.8rem;
		border-bottom: 1px solid var(--app-rule);
		background: var(--app-paper);
		font-size: 0.74rem;
	}

	.index-pagination p {
		margin: 0;
		color: var(--app-muted);
	}

	.index-pagination > div {
		display: flex;
		gap: 1rem;
	}

	@media (max-width: 980px) {
		.section-heading {
			display: grid;
			gap: 1.2rem;
		}
	}

	@media (max-width: 620px) {
		.index-pagination {
			align-items: flex-start;
			flex-direction: column;
		}
	}
</style>
