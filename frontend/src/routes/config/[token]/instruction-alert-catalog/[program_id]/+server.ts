import { json, type RequestHandler } from '@sveltejs/kit';
import { backendOriginRequest, readBackendError } from '$lib/server/backend';
import type { IdlInstructionProgramDescriptor } from '$lib/types';

export const GET: RequestHandler = async ({ cookies, fetch, params }) => {
	const token = cookies.get('firewatch_config_token')?.trim() || params.token?.trim() || '';
	const programId = params.program_id?.trim() ?? '';
	if (!token || !programId) {
		return json({ error: 'Program catalog request is incomplete.' }, { status: 400 });
	}

	const response = await backendOriginRequest(
		fetch,
		`/api/config-sessions/by-token/${encodeURIComponent(token)}/instruction-alert-catalog/${encodeURIComponent(programId)}`
	);
	if (!response.ok) {
		return json({ error: await readBackendError(response) }, { status: response.status });
	}

	return json((await response.json()) as IdlInstructionProgramDescriptor);
};
