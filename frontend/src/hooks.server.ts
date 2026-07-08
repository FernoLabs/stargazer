import { adminUnauthorizedResponse, verifyAdminAuthorization } from '$lib/server/admin-auth';
import type { Handle } from '@sveltejs/kit';

export const handle: Handle = async ({ event, resolve }) => {
	if (event.url.pathname === '/admin' || event.url.pathname.startsWith('/admin/')) {
		const authorization = await verifyAdminAuthorization(event.fetch, event.request);
		if (!authorization.authorized) {
			return adminUnauthorizedResponse(authorization);
		}
	}

	return resolve(event);
};
