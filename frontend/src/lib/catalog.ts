import {
	DISCOVERED_PROTOCOL_MULTISIG_LAST_SEEN_AT,
	DISCOVERED_PROTOCOL_MULTISIG_LINK_ROWS
} from '$lib/generated/protocolMultisigLinks';
import type {
	DefiProtocolDescriptor,
	ProtocolGovernanceLink,
	ProtocolMultisigLink,
	ProtocolProgramSurface
} from '$lib/types';

export const SQUADS_V4_PROGRAM_ID = 'SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf';

export interface CatalogProgramMatch {
	protocol: DefiProtocolDescriptor;
	program: ProtocolProgramSurface;
}

const DISCOVERED_PROTOCOL_MULTISIG_SOURCE = 'program_upgrade_authority';
const PROTOCOL_ALIAS_CANONICAL_SLUGS: Record<string, string> = {
	kamino_lend: 'kamino',
	meteora_dlmm: 'meteora'
};
const COVERAGE_RANK = {
	address_only: 0,
	admin_signals: 1,
	deep_decode: 2
} as const;

let discoveredProtocolMultisigLinksBySlug: Map<string, ProtocolMultisigLink[]> | null = null;

function protocolMultisigLinkKey(link: ProtocolMultisigLink) {
	return `${link.source}:${link.program_id}:${link.multisig}:${link.authority}`;
}

function protocolGovernanceLinkKey(link: ProtocolGovernanceLink) {
	return [
		link.source,
		link.governance_program_id,
		link.realm,
		link.governance,
		link.governed_account
	].join(':');
}

function uniqueBy<T>(items: T[], keyFor: (item: T) => string) {
	const seen = new Set<string>();
	const unique: T[] = [];
	for (const item of items) {
		const key = keyFor(item);
		if (seen.has(key)) continue;
		seen.add(key);
		unique.push(item);
	}
	return unique;
}

function programKey(program: ProtocolProgramSurface) {
	return [program.program_id, program.label, program.idl_metadata_address, program.anchor_legacy_idl_address ?? ''].join(
		':'
	);
}

function bestCoverage(left: DefiProtocolDescriptor['coverage'], right: DefiProtocolDescriptor['coverage']) {
	return COVERAGE_RANK[right] > COVERAGE_RANK[left] ? right : left;
}

export function canonicalProtocolSlug(slug: string) {
	return PROTOCOL_ALIAS_CANONICAL_SLUGS[slug] ?? slug;
}

export function protocolSlugAliases(slug: string) {
	const canonicalSlug = canonicalProtocolSlug(slug);
	return [
		canonicalSlug,
		...Object.entries(PROTOCOL_ALIAS_CANONICAL_SLUGS)
			.filter(([, canonical]) => canonical === canonicalSlug)
			.map(([alias]) => alias)
	];
}

function fallbackProtocolMultisigLinks() {
	if (discoveredProtocolMultisigLinksBySlug) return discoveredProtocolMultisigLinksBySlug;

	const linksBySlug = new Map<string, ProtocolMultisigLink[]>();

	for (const [
		protocolSlug,
		multisig,
		authority,
		programId,
		programLabel,
		vaultIndex
	] of DISCOVERED_PROTOCOL_MULTISIG_LINK_ROWS) {
		const links = linksBySlug.get(protocolSlug) ?? [];
		links.push({
			multisig,
			authority,
			program_id: programId,
			program_label: programLabel,
			evidence: `Program upgrade authority is Squads vault ${vaultIndex}`,
			source: DISCOVERED_PROTOCOL_MULTISIG_SOURCE,
			vault_index: vaultIndex,
			last_seen_at: DISCOVERED_PROTOCOL_MULTISIG_LAST_SEEN_AT
		});
		linksBySlug.set(protocolSlug, links);
	}

	discoveredProtocolMultisigLinksBySlug = linksBySlug;
	return linksBySlug;
}

export function hydrateProtocolRelatedMultisigs(
	protocols: DefiProtocolDescriptor[]
): DefiProtocolDescriptor[] {
	const fallbackLinks = fallbackProtocolMultisigLinks();
	let changed = false;

	const hydrated = protocols.map((protocol) => {
		const fallback = fallbackLinks.get(protocol.slug) ?? [];
		if (fallback.length === 0) return protocol;

		const relatedMultisigs: ProtocolMultisigLink[] = [];
		const seen = new Set<string>();

		for (const link of [...(protocol.related_multisigs ?? []), ...fallback]) {
			const key = protocolMultisigLinkKey(link);
			if (seen.has(key)) continue;
			seen.add(key);
			relatedMultisigs.push(link);
		}

		if (relatedMultisigs.length === (protocol.related_multisigs?.length ?? 0)) return protocol;
		changed = true;
		return {
			...protocol,
			related_multisigs: relatedMultisigs
		};
	});

	return changed ? hydrated : protocols;
}

export function mergeProtocolAliases(protocols: DefiProtocolDescriptor[]): DefiProtocolDescriptor[] {
	const groups = new Map<string, DefiProtocolDescriptor[]>();
	const order: string[] = [];

	for (const protocol of protocols) {
		const canonicalSlug = canonicalProtocolSlug(protocol.slug);
		if (!groups.has(canonicalSlug)) {
			groups.set(canonicalSlug, []);
			order.push(canonicalSlug);
		}
		groups.get(canonicalSlug)!.push(protocol);
	}

	let changed = false;
	const merged = order.map((slug) => {
		const group = groups.get(slug)!;
		const primary = group.find((protocol) => protocol.slug === slug) ?? group[0];
		if (group.length === 1 && primary.slug === slug) return primary;

		changed = true;
		return group.reduce<DefiProtocolDescriptor>(
			(acc, protocol) => ({
				...acc,
				slug,
				name: primary.name,
				program_id: primary.program_id,
				programs: uniqueBy([...acc.programs, ...protocol.programs], programKey),
				sol_domains: uniqueBy([...acc.sol_domains, ...protocol.sol_domains], (domain) =>
					domain.toLowerCase()
				),
				coverage: bestCoverage(acc.coverage, protocol.coverage),
				catalog_instruction_count: acc.catalog_instruction_count + protocol.catalog_instruction_count,
				admin_instruction_filters: uniqueBy(
					[...acc.admin_instruction_filters, ...protocol.admin_instruction_filters],
					(filter) => filter
				),
				high_impact_instruction_filters: uniqueBy(
					[...acc.high_impact_instruction_filters, ...protocol.high_impact_instruction_filters],
					(filter) => filter
				),
				amount_triggers: uniqueBy([...acc.amount_triggers, ...protocol.amount_triggers], (trigger) =>
					JSON.stringify(trigger)
				),
				unknown_alerts: acc.unknown_alerts || protocol.unknown_alerts,
				related_multisigs: uniqueBy(
					[...(acc.related_multisigs ?? []), ...(protocol.related_multisigs ?? [])],
					protocolMultisigLinkKey
				),
				related_governances: uniqueBy(
					[...(acc.related_governances ?? []), ...(protocol.related_governances ?? [])],
					protocolGovernanceLinkKey
				)
			}),
			{
				...primary,
				slug,
				name: primary.name,
				programs: [],
				sol_domains: [],
				catalog_instruction_count: 0,
				admin_instruction_filters: [],
				high_impact_instruction_filters: [],
				amount_triggers: [],
				related_multisigs: [],
				related_governances: []
			}
		);
	});

	return changed ? merged : protocols;
}

export function uniqueProtocolMultisigCount(protocol: DefiProtocolDescriptor) {
	return new Set((protocol.related_multisigs ?? []).map((link) => link.multisig).filter(Boolean)).size;
}

export function uniqueProtocolGovernanceCount(protocol: DefiProtocolDescriptor) {
	return new Set(
		(protocol.related_governances ?? [])
			.map((link) => link.realm || link.governance || link.governed_account)
			.filter(Boolean)
	).size;
}

export function protocolListedItemCount(protocol: DefiProtocolDescriptor) {
	return (
		protocol.programs.length +
		protocol.sol_domains.length +
		protocol.amount_triggers.length +
		uniqueProtocolMultisigCount(protocol) +
		uniqueProtocolGovernanceCount(protocol)
	);
}

export function hasProtocolListedItems(protocol: DefiProtocolDescriptor) {
	return protocolListedItemCount(protocol) > 0;
}

export function normalizedProtocolCatalog(protocols: DefiProtocolDescriptor[]) {
	return mergeProtocolAliases(hydrateProtocolRelatedMultisigs(protocols)).filter(hasProtocolListedItems);
}

export function syntheticSquadsDescriptor(): DefiProtocolDescriptor {
	return {
		slug: 'squads',
		name: 'Squads',
		program_id: SQUADS_V4_PROGRAM_ID,
		programs: [
			{
				label: 'v4 Multisig',
				program_id: SQUADS_V4_PROGRAM_ID,
				idl_metadata_address: '',
				anchor_legacy_idl_address: null
			}
		],
		sol_domains: [],
		coverage: 'admin_signals',
		catalog_instruction_count: 0,
		admin_instruction_filters: ['config_transaction_execute'],
		high_impact_instruction_filters: ['config_transaction_execute'],
		amount_triggers: [],
		unknown_alerts: false,
		related_multisigs: [],
		related_governances: []
	};
}

export function protocolDisplayName(protocols: DefiProtocolDescriptor[], slug: string) {
	if (slug === 'squads') return 'Squads';
	const canonicalSlug = canonicalProtocolSlug(slug);
	return protocols.find((protocol) => protocol.slug === canonicalSlug)?.name ?? slug.replaceAll('_', ' ');
}

export function findProgramMatch(
	protocols: DefiProtocolDescriptor[],
	address: string | null | undefined
): CatalogProgramMatch | null {
	if (!address) return null;
	const normalized = address.trim();

	for (const protocol of protocols) {
		const program = protocol.programs.find((candidate) => candidate.program_id === normalized);
		if (program) {
			return { protocol, program };
		}
	}

	return null;
}

export function protocolPageHref(slug: string) {
	return `/protocols/${canonicalProtocolSlug(slug)}`;
}

export function protocolActivityPageHref(slug: string) {
	return `/protocols/${canonicalProtocolSlug(slug)}/activity`;
}

export function programPageHref(address: string) {
	return `/programs/${address}`;
}
