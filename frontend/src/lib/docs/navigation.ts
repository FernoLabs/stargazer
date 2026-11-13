export type DocsStatus = 'Implemented' | 'In progress' | 'Planned';

export type DocsHref =
	| '/docs/architecture'
	| '/docs/overview'
	| '/docs/data-model'
	| '/docs/archive-indexer'
	| '/docs/live-ingestion'
	| '/docs/backfill'
	| '/docs/operations'
	| '/docs/roadmap';

/** Pages that describe the August 2026 proof of concept, not the current architecture. */
export const PROOF_OF_CONCEPT_GROUP = 'Proof of concept (August 2026)';

export type DocsSection = {
	id: string;
	label: string;
};

export type DocsNavigationItem = {
	href: DocsHref;
	label: string;
	description: string;
	keywords: readonly string[];
	sections: readonly DocsSection[];
};

export type DocsNavigationGroup = {
	label: string;
	items: readonly DocsNavigationItem[];
};

export const docsNavigation = [
	{
		label: 'Architecture',
		items: [
			{
				href: '/docs/architecture',
				label: 'Technical architecture',
				description:
					'Live ingestion, contract control history, SEP-48 decoding, detectors, delivery, GuardVault, and Sentinel.',
				keywords: [
					'architecture',
					'ingestion',
					'RPC',
					'control history',
					'SEP-48',
					'OpenZeppelin',
					'Blend',
					'detector',
					'webhook',
					'Telegram',
					'GuardVault',
					'Sentinel',
					'DeFindex',
					'vault',
					'threat model',
					'monitoring'
				],
				sections: [
					{ id: 'summary', label: '1. Summary' },
					{ id: 'what-exists-what-is-new', label: '2. What exists, what is new' },
					{ id: 'why-stellar', label: '3. Why Stellar' },
					{ id: 'overview', label: '4. Overview' },
					{ id: 'ledger-sources', label: '5. Ledger sources' },
					{ id: 'verification-and-atomic-commit', label: '6. Verification and atomic commit' },
					{ id: 'contract-control-history', label: '7. Contract control history' },
					{ id: 'attribution-and-decoding', label: '8. Attribution and decoding' },
					{ id: 'detector-catalogue', label: '9. Detector catalogue' },
					{ id: 'the-yieldblox-replay', label: '10. The YieldBlox replay' },
					{ id: 'ai-context', label: '11. AI context' },
					{ id: 'access-api-dashboard-status', label: '12. Access: API, dashboard, status' },
					{ id: 'subscriptions-and-delivery', label: '13. Subscriptions and delivery' },
					{ id: 'replayable-modules', label: '14. Replayable modules' },
					{ id: 'the-protection-vault-guardvault-and-sentinel', label: '15. GuardVault and Sentinel' },
					{ id: 'operations', label: '16. Operations' },
					{ id: 'guarantees', label: '17. Guarantees' },
					{ id: 'business-model', label: '18. Business model' },
					{ id: 'scope-boundaries-and-what-comes-next', label: '19. Scope boundaries' },
					{ id: 'threat-model-stride', label: '20. Threat model (STRIDE)' },
					{ id: 'monitoring-plan', label: '21. Monitoring plan' },
					{ id: 'environments-and-tests', label: '22. Environments and tests' },
					{ id: 'versions-november-2026', label: '23. Versions (November 2026)' },
					{ id: 'references', label: 'References' }
				]
			}
		]
	},
	{
		label: PROOF_OF_CONCEPT_GROUP,
		items: [
			{
				href: '/docs/overview',
				label: 'Overview',
				description: 'How the archive, indexer, database, live path, and backfill model fit together.',
				keywords: ['introduction', 'local-first', 'Stellar', 'system', 'documentation'],
				sections: [
					{ id: 'status-heading', label: 'Current system and target' },
					{ id: 'local-first-heading', label: 'What local-first means' },
					{ id: 'pipeline-heading', label: 'Archive-backed product path' },
					{ id: 'shared-core-heading', label: 'Historical and live paths' },
					{ id: 'backfill-heading', label: 'Adding an indexer' },
					{ id: 'reading-heading', label: 'Documentation map' }
				]
			},
			{
				href: '/docs/data-model',
				label: 'Data model',
				description: 'Evidence stores, exact wallet relationships, attribution, versions, and alert provenance.',
				keywords: ['SQLite', 'schema', 'wallet', 'contract', 'protocol', 'transaction', 'upgrade'],
				sections: [
					{ id: 'stores', label: 'Storage layers' },
					{ id: 'schema', label: 'Compact schema' },
					{ id: 'wallet-contract', label: 'Wallet-contract relationships' },
					{ id: 'transactions', label: 'Transaction identity' },
					{ id: 'attribution', label: 'Protocol attribution' },
					{ id: 'versions', label: 'Contract versions' },
					{ id: 'alerts', label: 'Alert provenance' }
				]
			},
			{
				href: '/docs/archive-indexer',
				label: 'Archive indexer',
				description: 'How local Galexie history becomes a compact, resumable evidence index.',
				keywords: ['Galexie', 'XDR', 'zstd', 'Rust', 'memory', 'compact index', 'finalizer'],
				sections: [
					{ id: 'pipeline', label: 'Data path' },
					{ id: 'local-first', label: 'Local archive mirror' },
					{ id: 'memory', label: 'Memory controls' },
					{ id: 'retention', label: 'Retention policy' },
					{ id: 'resume', label: 'Resume and continuity' },
					{ id: 'finalizer', label: 'Serving finalizer' }
				]
			},
			{
				href: '/docs/live-ingestion',
				label: 'Live ingestion',
				description: 'The planned recent-ledger tail, commit boundary, gap recovery, alerts, and multisig scope.',
				keywords: ['RPC', 'Captive Core', 'live', 'checkpoint', 'gap', 'multisig', 'alerts'],
				sections: [
					{ id: 'pipeline', label: 'Live pipeline' },
					{ id: 'sources', label: 'Source strategy' },
					{ id: 'checkpoint', label: 'Atomic checkpoint' },
					{ id: 'gaps', label: 'Gap recovery' },
					{ id: 'alerts', label: 'Detection and delivery' },
					{ id: 'multisig', label: 'Multisig boundary' }
				]
			},
			{
				href: '/docs/backfill',
				label: 'Backfill workflow',
				description: 'How a new parser moves from fixtures to archive replay, validation, and live activation.',
				keywords: ['parser', 'decoder', 'module', 'backfill', 'staging', 'promotion', 'fixtures'],
				sections: [
					{ id: 'lifecycle', label: 'Module lifecycle' },
					{ id: 'contract', label: 'Module contract' },
					{ id: 'fixtures', label: 'Regression fixtures' },
					{ id: 'staging', label: 'Staging and promotion' },
					{ id: 'validation', label: 'Validation gate' },
					{ id: 'delivery', label: 'Historical alert safety' }
				]
			},
			{
				href: '/docs/operations',
				label: 'Operations',
				description: 'Continuity rules, health signals, failure response, publication, and backup priorities.',
				keywords: ['operations', 'health', 'recovery', 'failure', 'backup', 'publication'],
				sections: [
					{ id: 'invariants', label: 'Pipeline invariants' },
					{ id: 'health', label: 'Health model' },
					{ id: 'failure-modes', label: 'Failure modes' },
					{ id: 'publication', label: 'Atomic publication' },
					{ id: 'backup', label: 'Backup priorities' }
				]
			},
			{
				href: '/docs/roadmap',
				label: 'Roadmap',
				description: 'Milestones for live ingestion, protocol coverage, signal sources, and delivery.',
				keywords: ['roadmap', 'protocols', 'signals', 'coverage', 'delivery', 'Telegram'],
				sections: [
					{ id: 'priorities', label: 'Technical milestones' },
					{ id: 'protocol-order', label: 'Protocol expansion' },
					{ id: 'signals', label: 'Signal expansion' },
					{ id: 'acceptance', label: 'Acceptance standard' }
				]
			}
		]
	}
] as const satisfies readonly DocsNavigationGroup[];

export const docsItems: readonly DocsNavigationItem[] = docsNavigation.reduce<
	DocsNavigationItem[]
>((items, group) => {
	items.push(...group.items);
	return items;
}, []);

export function getDocsGroup(item: DocsNavigationItem): DocsNavigationGroup {
	return docsNavigation.find((group) => group.items.some((candidate) => candidate.href === item.href)) ?? docsNavigation[0];
}
