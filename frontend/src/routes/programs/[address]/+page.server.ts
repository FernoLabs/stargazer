import { error } from '@sveltejs/kit';
import type {
	EventsResponse,
	ProgramIdlVersionsResponse,
	ProgramSignerStatsResponse,
	ProgramVersionsResponse
} from '$lib/types';
import { findProgramMatch, type CatalogProgramMatch } from '$lib/catalog';
import { backendRequest, readBackendError } from '$lib/server/backend';
import type { PageServerLoad } from './$types';

function shortAddress(address: string) {
	if (address.length < 12) return address;
	return `${address.slice(0, 4)}...${address.slice(-4)}`;
}

function rawProgramMatch(address: string): CatalogProgramMatch {
	const program = {
		label: `Program ${shortAddress(address)}`,
		program_id: address,
		idl_metadata_address: '',
		anchor_legacy_idl_address: null
	};

	return {
		protocol: {
			slug: 'program',
			name: 'Watched program',
			program_id: address,
			programs: [program],
			sol_domains: [],
			coverage: 'address_only',
			catalog_instruction_count: 0,
			admin_instruction_filters: [],
			high_impact_instruction_filters: [],
			amount_triggers: [],
			unknown_alerts: false
		},
		program
	};
}

export const load = (async ({ fetch, params, parent }) => {
	const app = await parent();
	const address = params.address.trim();
	const catalogMatch = findProgramMatch(app.defiProtocols, address);
	const match = catalogMatch ?? rawProgramMatch(address);

	if (!address) {
		throw error(404, 'Program not found');
	}

	let events: EventsResponse['items'] = [];
	let nextOffset: number | null = null;
	let programEventsError: string | null = null;
	let signerStats: ProgramSignerStatsResponse['items'] = [];
	let signerStatsError: string | null = null;
	let versions: ProgramVersionsResponse['items'] = [];
	let versionsError: string | null = null;
	let idlVersions: ProgramIdlVersionsResponse['items'] = [];
	let idlVersionsError: string | null = null;

	try {
		const search = new URLSearchParams({
			limit: '120',
			target_address: match.program.program_id,
			scope: 'programs'
		});

		const eventsResponse = await backendRequest(fetch, `/api/events?${search.toString()}`);
		if (!eventsResponse.ok) throw new Error(await readBackendError(eventsResponse));

		const payload = (await eventsResponse.json()) as EventsResponse;
		events = payload.items;
		nextOffset = payload.next_offset;
	} catch (err) {
		programEventsError = err instanceof Error ? err.message : 'Unknown backend error';
	}

	try {
		const signerStatsResponse = await backendRequest(
			fetch,
			`/api/programs/${match.program.program_id}/signer-stats`
		);
		if (!signerStatsResponse.ok) throw new Error(await readBackendError(signerStatsResponse));
		signerStats = ((await signerStatsResponse.json()) as ProgramSignerStatsResponse).items;
	} catch (err) {
		signerStatsError = err instanceof Error ? err.message : 'Unknown backend error';
	}

	try {
		const versionsResponse = await backendRequest(
			fetch,
			`/api/programs/${match.program.program_id}/versions`
		);
		if (!versionsResponse.ok) throw new Error(await readBackendError(versionsResponse));
		versions = ((await versionsResponse.json()) as ProgramVersionsResponse).items;
	} catch (err) {
		versionsError = err instanceof Error ? err.message : 'Unknown backend error';
	}

	try {
		const idlVersionsResponse = await backendRequest(
			fetch,
			`/api/programs/${match.program.program_id}/idls`
		);
		if (!idlVersionsResponse.ok) throw new Error(await readBackendError(idlVersionsResponse));
		idlVersions = ((await idlVersionsResponse.json()) as ProgramIdlVersionsResponse).items;
	} catch (err) {
		idlVersionsError = err instanceof Error ? err.message : 'Unknown backend error';
	}

	return {
		protocol: match.protocol,
		program: match.program,
		events,
		nextOffset,
		programEventsError,
		signerStats,
		signerStatsError,
		versions,
		versionsError,
		idlVersions,
		idlVersionsError,
		catalogedProgram: Boolean(catalogMatch)
	};
}) satisfies PageServerLoad;
