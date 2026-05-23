import { env as privateEnv } from '$env/dynamic/private';

const DEFAULT_ADMIN_USERNAME = 'admin';

export function isAdminAuthConfigured() {
	return Boolean(adminPassword());
}

export function isAdminAuthorized(request: Request) {
	const expectedPassword = adminPassword();
	if (!expectedPassword) return false;

	const authorization = request.headers.get('authorization') ?? '';
	if (!authorization.startsWith('Basic ')) return false;

	const credentials = decodeBasicCredentials(authorization.slice('Basic '.length).trim());
	if (!credentials) return false;

	return (
		secureCompare(credentials.username, adminUsername()) &&
		secureCompare(credentials.password, expectedPassword)
	);
}

export function adminUnauthorizedResponse() {
	if (!isAdminAuthConfigured()) {
		return new Response('FireWatch admin auth is not configured.', {
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

function adminUsername() {
	return privateEnv.ADMIN_USERNAME?.trim() || DEFAULT_ADMIN_USERNAME;
}

function adminPassword() {
	return privateEnv.ADMIN_PASSWORD?.trim() ?? '';
}

function decodeBasicCredentials(encoded: string) {
	try {
		const decoded = atob(encoded);
		const separator = decoded.indexOf(':');
		if (separator === -1) return null;

		return {
			username: decoded.slice(0, separator),
			password: decoded.slice(separator + 1)
		};
	} catch {
		return null;
	}
}

function secureCompare(left: string, right: string) {
	const encoder = new TextEncoder();
	const leftBytes = encoder.encode(left);
	const rightBytes = encoder.encode(right);
	let diff = leftBytes.length ^ rightBytes.length;
	const length = Math.max(leftBytes.length, rightBytes.length);

	for (let index = 0; index < length; index += 1) {
		diff |= (leftBytes[index] ?? 0) ^ (rightBytes[index] ?? 0);
	}

	return diff === 0;
}
