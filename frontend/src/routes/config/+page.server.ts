import { dev } from '$app/environment';
import { redirect } from '@sveltejs/kit';
import { backendOriginRequest, readBackendError } from '$lib/server/backend';
import type { PageServerLoad } from './$types';

const CONFIG_COOKIE_NAME = 'firewatch_config_token';

export const load = (async ({ cookies, fetch }) => {
	const token = cookies.get(CONFIG_COOKIE_NAME)?.trim();
	if (!token) {
		return {
			error: null
		};
	}

	const response = await backendOriginRequest(fetch, `/api/config-sessions/by-token/${token}`);
	if (response.ok) {
		redirect(303, `/config/${token}`);
	}

	cookies.delete(CONFIG_COOKIE_NAME, {
		path: '/',
		secure: !dev
	});

	return {
		error: await readBackendError(response)
	};
}) satisfies PageServerLoad;
