import { error } from '@sveltejs/kit';
import { backendOriginRequest, readBackendError } from '$lib/server/backend';
import type { PaymentLinkCheckoutResponse } from '$lib/types';
import type { PageServerLoad } from './$types';

export const load = (async ({ fetch, params, cookies }) => {
	const token = params.token.trim();
	if (!token) {
		error(404, 'Payment link not found');
	}

	const response = await backendOriginRequest(fetch, `/api/payment-links/by-token/${token}`);
	if (!response.ok) {
		error(response.status, await readBackendError(response));
	}

	cookies.set('firewatch_payment_token', token, {
		httpOnly: true,
		sameSite: 'lax',
		secure: true,
		path: '/',
		maxAge: 60 * 60 * 24 * 7
	});

	return (await response.json()) as PaymentLinkCheckoutResponse;
}) satisfies PageServerLoad;
