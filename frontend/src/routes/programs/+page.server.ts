import { backendRequest, readBackendError } from '$lib/server/backend';
import type { ProgramVersionRecord, ProgramVersionsResponse } from '$lib/types';
import type { PageServerLoad } from './$types';

type ProgramVersionSummary = {
	program_id: string;
	version_count: number;
	diffable_count: number;
	incomplete_count: number;
	latest: Pick<
		ProgramVersionRecord,
		| 'id'
		| 'kind'
		| 'slot'
		| 'block_time'
		| 'complete'
		| 'program_len'
		| 'program_sha256'
		| 'previous_version_id'
	> | null;
	error: string | null;
};

function latestVersion(versions: ProgramVersionRecord[]) {
	return [...versions].sort(
		(left, right) =>
			right.slot - left.slot || (right.block_time ?? 0) - (left.block_time ?? 0)
	)[0] ?? null;
}

async function loadProgramVersionSummary(
	fetch: typeof globalThis.fetch,
	programId: string
): Promise<ProgramVersionSummary> {
	try {
		const response = await backendRequest(fetch, `/api/programs/${programId}/versions`);
		if (!response.ok) throw new Error(await readBackendError(response));

		const versions = ((await response.json()) as ProgramVersionsResponse).items;
		const latest = latestVersion(versions);

		return {
			program_id: programId,
			version_count: versions.length,
			diffable_count: versions.filter((version) => version.complete && version.previous_version_id)
				.length,
			incomplete_count: versions.filter((version) => !version.complete).length,
			latest: latest
				? {
						id: latest.id,
						kind: latest.kind,
						slot: latest.slot,
						block_time: latest.block_time,
						complete: latest.complete,
						program_len: latest.program_len,
						program_sha256: latest.program_sha256,
						previous_version_id: latest.previous_version_id
					}
				: null,
			error: null
		};
	} catch (err) {
		return {
			program_id: programId,
			version_count: 0,
			diffable_count: 0,
			incomplete_count: 0,
			latest: null,
			error: err instanceof Error ? err.message : 'Unknown backend error'
		};
	}
}

export const load = (async ({ fetch, parent }) => {
	const app = await parent();
	const programIds = [
		...new Set(
			app.defiProtocols.flatMap((protocol) =>
				protocol.programs.map((program) => program.program_id)
			)
		)
	];

	const summaries = await Promise.all(
		programIds.map((programId) => loadProgramVersionSummary(fetch, programId))
	);

	return {
		programVersionSummaries: Object.fromEntries(
			summaries.map((summary) => [summary.program_id, summary])
		)
	};
}) satisfies PageServerLoad;
