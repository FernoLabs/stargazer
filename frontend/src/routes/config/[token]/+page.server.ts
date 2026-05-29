import { fail, redirect } from '@sveltejs/kit';
import { backendOriginRequest, readBackendError } from '$lib/server/backend';
import type { PaymentLink, TelegramConfigSessionResponse } from '$lib/types';
import type { Actions, PageServerLoad } from './$types';

export const load = (async ({ fetch, params, cookies }) => {
	const token = params.token.trim();
	if (!token) {
		redirect(303, '/');
	}

	const response = await backendOriginRequest(fetch, `/api/config-sessions/by-token/${token}`);
	if (!response.ok) {
		return {
			error: await readBackendError(response),
			session: null,
			user: null
		};
	}

	cookies.set('firewatch_config_token', token, {
		httpOnly: true,
		sameSite: 'lax',
		secure: true,
		path: '/',
		maxAge: 15 * 60
	});

	const payload = (await response.json()) as TelegramConfigSessionResponse;
	return {
		error: null,
		session: payload.session,
		user: payload.user
	};
}) satisfies PageServerLoad;

export const actions = {
	create_checkout: async ({ request, fetch, params }) => {
		const token = params.token.trim();
		const form = await request.formData();
		const billing = String(form.get('billing') ?? 'month');

		const sessionResponse = await backendOriginRequest(
			fetch,
			`/api/config-sessions/by-token/${token}`
		);
		if (!sessionResponse.ok) {
			return fail(sessionResponse.status, {
				error: await readBackendError(sessionResponse)
			});
		}

		const sessionPayload = (await sessionResponse.json()) as TelegramConfigSessionResponse;
		const response = await backendOriginRequest(fetch, '/api/payment-links', {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify({
				user_id: sessionPayload.user.id,
				plan_id: billing === 'year' ? 'paid-year' : 'paid',
				amount: null
			})
		});

		if (!response.ok) {
			return fail(response.status, {
				error: await readBackendError(response)
			});
		}

		const paymentLink = (await response.json()) as PaymentLink;
		redirect(303, paymentLink.public_url);
	}
} satisfies Actions;
