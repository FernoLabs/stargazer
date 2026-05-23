import type { DefiProtocolDescriptor, ProtocolProgramSurface } from '$lib/types';

export const SQUADS_V4_PROGRAM_ID = 'SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf';

export interface CatalogProgramMatch {
	protocol: DefiProtocolDescriptor;
	program: ProtocolProgramSurface;
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
		unknown_alerts: false
	};
}

export function protocolDisplayName(protocols: DefiProtocolDescriptor[], slug: string) {
	if (slug === 'squads') return 'Squads';
	return protocols.find((protocol) => protocol.slug === slug)?.name ?? slug.replaceAll('_', ' ');
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
	return `/protocols/${slug}`;
}

export function programPageHref(address: string) {
	return `/programs/${address}`;
}
