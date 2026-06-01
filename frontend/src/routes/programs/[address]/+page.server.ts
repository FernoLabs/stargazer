import { error } from '@sveltejs/kit';
import { nextPageOffset } from '$lib/api-pagination';
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

	const eventsPromise = (async () => {
		const search = new URLSearchParams({
			'page[size]': '120',
			target_address: match.program.program_id,
			scope: 'programs'
		});

		const eventsResponse = await backendRequest(fetch, `/api/events?${search.toString()}`);
		if (!eventsResponse.ok) throw new Error(await readBackendError(eventsResponse));

		const payload = (await eventsResponse.json()) as EventsResponse;
		return {
			events: payload.items,
			nextOffset: nextPageOffset(payload.links),
			programEventsError: null
		};
	})().catch((err) => ({
		events: [] as EventsResponse['items'],
		nextOffset: null,
		programEventsError: err instanceof Error ? err.message : 'Unknown backend error'
	}));

	const signerStatsPromise = (async () => {
		const signerStatsResponse = await backendRequest(
			fetch,
			`/api/programs/${match.program.program_id}/signer-stats`
		);
		if (!signerStatsResponse.ok) throw new Error(await readBackendError(signerStatsResponse));
		return {
			signerStats: ((await signerStatsResponse.json()) as ProgramSignerStatsResponse).items,
			signerStatsError: null
		};
	})().catch((err) => ({
		signerStats: [] as ProgramSignerStatsResponse['items'],
		signerStatsError: err instanceof Error ? err.message : 'Unknown backend error'
	}));

	const versionsPromise = (async () => {
		const versionsResponse = await backendRequest(
			fetch,
			`/api/programs/${match.program.program_id}/versions`
		);
		if (!versionsResponse.ok) throw new Error(await readBackendError(versionsResponse));
		return {
			versions: ((await versionsResponse.json()) as ProgramVersionsResponse).items,
			versionsError: null
		};
	})().catch((err) => ({
		versions: [] as ProgramVersionsResponse['items'],
		versionsError: err instanceof Error ? err.message : 'Unknown backend error'
	}));

	const idlVersionsPromise = (async () => {
		const idlVersionsResponse = await backendRequest(
			fetch,
			`/api/programs/${match.program.program_id}/idls`
		);
		if (!idlVersionsResponse.ok) throw new Error(await readBackendError(idlVersionsResponse));
		return {
			idlVersions: ((await idlVersionsResponse.json()) as ProgramIdlVersionsResponse).items,
			idlVersionsError: null
		};
	})().catch((err) => ({
		idlVersions: [] as ProgramIdlVersionsResponse['items'],
		idlVersionsError: err instanceof Error ? err.message : 'Unknown backend error'
	}));

	const [
		{ events, nextOffset, programEventsError },
		{ signerStats, signerStatsError },
		{ versions, versionsError },
		{ idlVersions, idlVersionsError }
	] = await Promise.all([
		eventsPromise,
		signerStatsPromise,
		versionsPromise,
		idlVersionsPromise
	]);

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
