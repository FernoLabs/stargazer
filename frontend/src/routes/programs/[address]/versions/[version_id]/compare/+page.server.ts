import { error } from '@sveltejs/kit';
import { findProgramMatch, type CatalogProgramMatch } from '$lib/catalog';
import { backendRequest, readBackendError } from '$lib/server/backend';
import type {
	ProgramDisassemblyRecord,
	ProgramDisassemblyResponse,
	ProgramVersionsResponse
} from '$lib/types';
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

async function fetchDisassembly(
	fetch: typeof globalThis.fetch,
	path: string
): Promise<ProgramDisassemblyRecord | null> {
	const response = await backendRequest(fetch, path);
	if (response.status === 404) return null;
	if (!response.ok) throw new Error(await readBackendError(response));
	return ((await response.json()) as ProgramDisassemblyResponse).item;
}

export const load = (async ({ fetch, params, parent }) => {
	const app = await parent();
	const address = params.address.trim();
	const versionId = params.version_id;
	if (!address || !versionId) throw error(404, 'Program version not found');

	const catalogMatch = findProgramMatch(app.defiProtocols, address);
	const match = catalogMatch ?? rawProgramMatch(address);

	const versionsResponse = await backendRequest(fetch, `/api/programs/${address}/versions`);
	if (!versionsResponse.ok) throw error(versionsResponse.status, await readBackendError(versionsResponse));
	const versions = ((await versionsResponse.json()) as ProgramVersionsResponse).items;
	const version = versions.find((candidate) => candidate.id === versionId);
	if (!version) throw error(404, 'Program version not found');

	let beforeDisassembly: ProgramDisassemblyRecord | null = null;
	let afterDisassembly: ProgramDisassemblyRecord | null = null;
	let disassemblyError: string | null = null;

	try {
		const basePath = `/api/programs/${address}/versions/${encodeURIComponent(versionId)}/disassembly`;
		[beforeDisassembly, afterDisassembly] = await Promise.all([
			version.previous_version_id
				? fetchDisassembly(fetch, `${basePath}?side=before`)
				: Promise.resolve(null),
			fetchDisassembly(fetch, `${basePath}?side=after`)
		]);
	} catch (err) {
		disassemblyError = err instanceof Error ? err.message : 'Unknown backend error';
	}

	return {
		protocol: match.protocol,
		program: match.program,
		catalogedProgram: Boolean(catalogMatch),
		version,
		beforeDisassembly,
		afterDisassembly,
		disassemblyError
	};
}) satisfies PageServerLoad;
