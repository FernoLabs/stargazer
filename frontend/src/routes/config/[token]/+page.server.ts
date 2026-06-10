import { fail, redirect } from '@sveltejs/kit';
import { backendOriginRequest, readBackendError } from '$lib/server/backend';
import { customPlanAmountForBilling, customPlanPricing } from '$lib/server/custom-plan-pricing';
import type { PaymentLink, SaveTelegramConfigResponse, TelegramConfigSessionResponse } from '$lib/types';
import type { Actions, PageServerLoad } from './$types';

const SOLANA_ADDRESS_PATTERN = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;

function normalizedWalletAddress(value: string) {
	const trimmed = value.trim();
	if (!trimmed) return null;
	return SOLANA_ADDRESS_PATTERN.test(trimmed) ? trimmed : null;
}

function parseWalletAddressLines(raw: string) {
	const tokens = raw
		.split(/[\n,]+/)
		.map((value) => value.trim())
		.filter(Boolean);
	const seen = new Set<string>();
	const addresses: string[] = [];
	const invalid: string[] = [];

	for (const token of tokens) {
		const normalized = normalizedWalletAddress(token);
		if (!normalized) {
			invalid.push(token);
			continue;
		}
		if (seen.has(normalized)) continue;
		seen.add(normalized);
		addresses.push(normalized);
	}

	if (invalid.length > 0) {
		throw new Error(
			`${invalid.length} invalid wallet ${invalid.length === 1 ? 'address was' : 'addresses were'} provided.`
		);
	}

	return addresses;
}

function parseWalletAddressbook(raw: string) {
	const payload = JSON.parse(raw) as unknown;
	const items = Array.isArray(payload)
		? payload
		: Array.isArray((payload as { items?: unknown } | null)?.items)
			? ((payload as { items: unknown[] }).items ?? [])
			: null;

	if (!items) {
		throw new Error('Wallet JSON import expects an items array.');
	}

	const seen = new Set<string>();
	const addresses: string[] = [];
	const invalid: string[] = [];

	for (const entry of items) {
		if (!entry || typeof entry !== 'object') continue;
		const address = normalizedWalletAddress(String((entry as { address?: unknown }).address ?? ''));
		if (!address) {
			invalid.push(String((entry as { address?: unknown }).address ?? ''));
			continue;
		}
		if (seen.has(address)) continue;
		seen.add(address);
		addresses.push(address);
	}

	if (invalid.length > 0) {
		throw new Error(
			`Wallet JSON contains ${invalid.length} invalid ${invalid.length === 1 ? 'entry' : 'entries'}.`
		);
	}

	return addresses;
}

function parseWalletAddresses(raw: string) {
	const trimmed = raw.trim();
	if (!trimmed) return [];

	if (trimmed.startsWith('{') || trimmed.startsWith('[')) {
		return parseWalletAddressbook(trimmed);
	}

	return parseWalletAddressLines(trimmed);
}

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
		user: payload.user,
		billing: payload.billing,
		customPlanPricing: customPlanPricing()
	};
}) satisfies PageServerLoad;

export const actions = {
	save_config: async ({ request, fetch, params }) => {
		const token = params.token.trim();
		const form = await request.formData();
		let walletAddresses: string[] = [];
		try {
			walletAddresses = parseWalletAddresses(String(form.get('wallet_addresses') ?? ''));
		} catch (error) {
			return fail(400, {
				error: error instanceof Error ? error.message : 'Wallet address parsing failed.'
			});
		}
		const solDomains = String(form.get('sol_domains') ?? '')
			.split(/[\n,]+/)
			.map((value) => value.trim())
			.filter(Boolean);
		const billing = String(form.get('billing') ?? 'month');

		const response = await backendOriginRequest(
			fetch,
			`/api/config-sessions/by-token/${token}/save`,
			{
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({
					protocol_slugs: form.getAll('protocol_slugs').map(String),
					program_ids: form.getAll('program_ids').map(String),
					wallet_addresses: walletAddresses,
					sol_domains: solDomains,
					billing
				})
			}
		);

		if (!response.ok) {
			return fail(response.status, {
				error: await readBackendError(response)
			});
		}

		const payload = (await response.json()) as SaveTelegramConfigResponse;
		if (payload.payment_link) {
			redirect(303, payload.payment_link.public_url);
		}

		return {
			message: `${payload.saved_count} watch ${payload.saved_count === 1 ? 'rule' : 'rules'} saved.`
		};
	},
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
				amount: customPlanAmountForBilling(billing)
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
