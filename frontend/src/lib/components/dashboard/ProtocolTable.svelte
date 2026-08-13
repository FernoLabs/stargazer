<script lang="ts">
	import ContractIdenticon from '$lib/components/dashboard/ContractIdenticon.svelte';
	import ProtocolLogo from '$lib/components/dashboard/ProtocolLogo.svelte';
	import { formatAddress, formatExactCount } from '$lib/dashboard/format';
	import type {
		LiveContractPage,
		LiveProtocolRelationship
	} from '$lib/dashboard/live';
	import { protocolDisplayName } from '$lib/dashboard/protocols';

	interface Props {
		protocols: readonly LiveProtocolRelationship[];
		contracts: LiveContractPage;
	}

	let { protocols, contracts }: Props = $props();
	let unattributedContracts = $derived(
		contracts.items.filter((contract) => contract.attributionStatus === 'unattributed')
	);
	let hasRows = $derived(protocols.length > 0 || unattributedContracts.length > 0);

	const dateFormatter = new Intl.DateTimeFormat('en-GB', {
		day: '2-digit',
		month: 'short',
		year: 'numeric',
		timeZone: 'UTC'
	});

	function formatDate(value: string): string {
		return dateFormatter.format(new Date(value));
	}
</script>

{#if !hasRows}
	<div class="empty-protocols">
		<strong>No contract relationship found</strong>
		<p>
			The exact wallet ↔ contract index has no relationship for this wallet in the indexed
			ledger range.
		</p>
	</div>
{:else}
	<div class="table-scroll">
		<table>
			<thead>
				<tr>
					<th scope="col">Protocol or contract</th>
					<th scope="col">Transactions</th>
					<th scope="col">Contracts</th>
					<th scope="col">First observed</th>
					<th scope="col">Attribution</th>
				</tr>
			</thead>
			<tbody>
				{#each protocols as protocol (`protocol:${protocol.protocolId}`)}
					<tr>
						<th scope="row" data-label="Dependency">
							<div class="protocol-identity">
								<ProtocolLogo protocolId={protocol.protocolId} />
								<div>
									<strong>{protocolDisplayName(protocol.protocolId)}</strong>
									<code>{protocol.protocolId}</code>
								</div>
							</div>
						</th>
						<td data-label="Transactions">
							{#if protocol.transactionCount === null}
								<strong class="unavailable">Not indexed</strong>
								<span class="submetric">wallet-wide totals remain available separately</span>
							{:else}
								<strong class="metric">{formatExactCount(protocol.transactionCount)}</strong>
								<span class="submetric">
									{protocol.successfulTransactionCount === null
										? 'success total unavailable'
										: `${formatExactCount(protocol.successfulTransactionCount)} successful`}
								</span>
							{/if}
						</td>
						<td data-label="Contracts">
							<details>
								<summary>
									{protocol.contractCount} contract{protocol.contractCount === 1 ? '' : 's'}
								</summary>
								<ul>
									{#each protocol.contracts as contract (contract)}
										<li title={contract}>{formatAddress(contract, { start: 8, end: 6 })}</li>
									{/each}
								</ul>
							</details>
						</td>
						<td data-label="First observed">
							{#if protocol.firstInteractionAt}
								<time datetime={protocol.firstInteractionAt}>{formatDate(protocol.firstInteractionAt)}</time>
							{:else}
								<span class="unavailable">Date unavailable</span>
							{/if}
							<span class="submetric">ledger {formatExactCount(protocol.firstInteractionLedger)}</span>
						</td>
						<td data-label="Attribution">
							<strong class="coverage">Verified registry</strong>
							<span class="submetric">source-backed contract match</span>
						</td>
					</tr>
				{/each}
				{#each unattributedContracts as contract (`contract:${contract.contractAddress}`)}
					<tr>
						<th scope="row" data-label="Dependency">
							<div class="protocol-identity">
								<ContractIdenticon contractAddress={contract.placeholderProtocolId} />
								<div>
									<strong title={contract.placeholderProtocolId}>
										{formatAddress(contract.placeholderProtocolId, { start: 10, end: 8 })}
									</strong>
									<code>Unattributed contract</code>
								</div>
							</div>
						</th>
						<td data-label="Transactions">
							<strong class="unavailable">Not indexed per contract</strong>
							<span class="submetric">wallet-wide totals remain available separately</span>
						</td>
						<td data-label="Contract">
							<code class="contract-address" title={contract.placeholderProtocolId}>
								{formatAddress(contract.placeholderProtocolId, { start: 8, end: 6 })}
							</code>
						</td>
						<td data-label="First observed">
							{#if contract.firstInteractionAt}
								<time datetime={contract.firstInteractionAt}>{formatDate(contract.firstInteractionAt)}</time>
							{:else}
								<span class="unavailable">Date unavailable</span>
							{/if}
							<span class="submetric">ledger {formatExactCount(contract.firstInteractionLedger)}</span>
						</td>
						<td data-label="Attribution">
							<strong class="unattributed">Unattributed</strong>
							<span class="submetric">on-chain address used as placeholder</span>
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
{/if}

<style>
	.table-scroll,
	.empty-protocols {
		border-top: 1px solid var(--app-ink);
	}

	.table-scroll {
		overflow-x: auto;
		-webkit-overflow-scrolling: touch;
	}

	.empty-protocols {
		padding: 1.5rem;
		border-bottom: 1px solid var(--app-rule);
		background: var(--app-paper);
	}

	.empty-protocols p {
		max-width: 46rem;
		margin: 0.3rem 0 0;
		color: var(--app-muted);
		line-height: 1.5;
	}

	table {
		width: 100%;
		min-width: 820px;
		border-collapse: collapse;
		background: var(--app-paper);
		font-size: 0.78rem;
		text-align: left;
	}

	thead {
		background: #f8f8f5;
	}

	th,
	td {
		padding: 0.9rem 0.8rem;
		border-bottom: 1px solid var(--app-rule);
		vertical-align: top;
	}

	thead th {
		color: var(--app-muted);
		font-size: 0.67rem;
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
		white-space: nowrap;
	}

	tbody th {
		min-width: 13rem;
		font-weight: 400;
	}

	.protocol-identity {
		display: flex;
		gap: 0.65rem;
		align-items: center;
	}

	.protocol-identity > div {
		min-width: 0;
	}

	tbody th strong,
	.metric,
	.coverage {
		display: block;
		font-weight: 600;
	}

	tbody th code,
	.submetric {
		display: block;
		margin-top: 0.18rem;
		color: var(--app-muted);
		font-size: 0.66rem;
	}

	.metric,
	time,
	code,
	details,
	.unavailable {
		font-family: 'Fragment Mono', monospace;
	}

	.unavailable {
		color: var(--app-muted);
		font-size: 0.7rem;
		font-weight: 500;
	}

	details {
		position: relative;
		font-size: 0.68rem;
	}

	summary {
		color: var(--app-violet);
		cursor: pointer;
		white-space: nowrap;
	}

	details ul {
		position: absolute;
		z-index: 2;
		width: 13rem;
		margin: 0.45rem 0 0;
		padding: 0.5rem;
		border: 1px solid var(--app-rule-strong);
		background: var(--app-paper);
		list-style: none;
	}

	details li + li {
		margin-top: 0.35rem;
	}

	.coverage {
		color: var(--app-success);
	}

	.unattributed {
		display: block;
		color: var(--app-muted);
		font-weight: 600;
	}

	.contract-address {
		font-size: 0.68rem;
	}

	@media (max-width: 700px) {
		.table-scroll {
			overflow: visible;
		}

		table,
		tbody,
		tr,
		th,
		td {
			display: block;
			width: 100%;
			min-width: 0;
			box-sizing: border-box;
		}

		thead {
			display: none;
		}

		tbody tr {
			padding: 0.75rem 0;
			border-bottom: 1px solid var(--app-rule-strong);
		}

		th,
		td {
			display: grid;
			grid-template-columns: 7.5rem minmax(0, 1fr);
			gap: 0.75rem;
			padding: 0.45rem 0.8rem;
			border-bottom: 0;
		}

		th::before,
		td::before {
			color: var(--app-muted);
			content: attr(data-label);
			font-size: 0.66rem;
			font-weight: 600;
			letter-spacing: 0.03em;
			text-transform: uppercase;
		}

		details,
		details ul {
			position: static;
		}

		details ul {
			width: auto;
		}
	}
</style>
