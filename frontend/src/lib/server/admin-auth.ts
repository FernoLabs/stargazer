import { backendOriginRequest, readBackendError } from '$lib/server/backend';

type AdminAuthorizationReason = 'authorized' | 'missing-config' | 'unauthorized' | 'unavailable';

export type AdminAuthorization = {
	authorized: boolean;
	reason: AdminAuthorizationReason;
};

const AUTH_CHECK_TIMEOUT_MS = 5000;

export async function verifyAdminAuthorization(
	fetch: typeof globalThis.fetch,
	request: Request
): Promise<AdminAuthorization> {
	const controller = new AbortController();
	const timeout = setTimeout(() => controller.abort(), AUTH_CHECK_TIMEOUT_MS);
	const authorization = request.headers.get('authorization') ?? '';
	const headers = authorization ? { authorization } : undefined;

	try {
		const response = await backendOriginRequest(fetch, '/api/admin/auth/check', {
			headers,
			signal: controller.signal
		});

		if (response.ok) {
			return { authorized: true, reason: 'authorized' };
		}

		if (response.status === 503) {
			return { authorized: false, reason: 'missing-config' };
		}

		if (response.status === 401) {
			const message = await readBackendError(response);
			return {
				authorized: false,
				reason: message.toLowerCase().includes('origin token') ? 'unavailable' : 'unauthorized'
			};
		}
	} catch {
		return { authorized: false, reason: 'unavailable' };
	} finally {
		clearTimeout(timeout);
	}

	return { authorized: false, reason: 'unavailable' };
}

export function adminUnauthorizedResponse(authorization?: AdminAuthorization) {
	if (authorization?.reason === 'missing-config') {
		return new Response('FireWatch admin auth is not configured.', {
			status: 503,
			headers: {
				'cache-control': 'no-store'
			}
		});
	}

	if (authorization?.reason === 'unavailable') {
		return new Response('FireWatch admin auth is unavailable.', {
			status: 503,
			headers: {
				'cache-control': 'no-store'
			}
		});
	}

	return new Response('Authentication required.', {
		status: 401,
		headers: {
			'cache-control': 'no-store',
			'www-authenticate': 'Basic realm="FireWatch Admin", charset="UTF-8"'
		}
	});
}
