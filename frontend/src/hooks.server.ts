import { adminUnauthorizedResponse, isAdminAuthorized } from '$lib/server/admin-auth';
import type { Handle } from '@sveltejs/kit';

export const handle: Handle = async ({ event, resolve }) => {
	if (event.url.pathname === '/admin' || event.url.pathname.startsWith('/admin/')) {
		if (!isAdminAuthorized(event.request)) {
			return adminUnauthorizedResponse();
		}
	}

	return resolve(event);
};
