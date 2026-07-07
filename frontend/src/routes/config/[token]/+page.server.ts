import { dev } from '$app/environment';
import { fail, redirect } from '@sveltejs/kit';
import { backendOriginRequest, readBackendError } from '$lib/server/backend';
import { customPlanAmountForBilling, customPlanPricing } from '$lib/server/custom-plan-pricing';
import type {
	MetricAlertsResponse,
	PaymentLink,
	RememberTelegramConfigSessionResponse,
	SaveTelegramConfigResponse,
	TelegramConfigSessionResponse
} from '$lib/types';
import type { Cookies } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';

const SOLANA_ADDRESS_PATTERN = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;
const CONFIG_COOKIE_NAME = 'firewatch_config_token';
const REMEMBERED_CONFIG_COOKIE_SECONDS = 30 * 24 * 60 * 60;
const REMEMBER_RENEW_BEFORE_SECONDS = 7 * 24 * 60 * 60;

function configCookieOptions(maxAge: number) {
	return {
		httpOnly: true,
		sameSite: 'lax' as const,
		secure: !dev,
		path: '/',
		maxAge
	};
}

function rememberConfigCookie(cookies: Cookies, token: string, expiresAt: number) {
	const maxAge = Math.min(
		REMEMBERED_CONFIG_COOKIE_SECONDS,
		Math.max(0, expiresAt - Math.floor(Date.now() / 1000))
	);
	if (maxAge <= 0) return;
	cookies.set(CONFIG_COOKIE_NAME, token, configCookieOptions(maxAge));
}

function forgetConfigCookie(cookies: Cookies) {
	cookies.delete(CONFIG_COOKIE_NAME, { path: '/' });
}

function rememberedToken(cookies: Cookies) {
	return cookies.get(CONFIG_COOKIE_NAME)?.trim() || null;
}

function actionToken(cookies: Cookies, routeToken: string) {
	return rememberedToken(cookies) ?? routeToken.trim();
}

async function loadConfigSession(fetch: typeof globalThis.fetch, token: string) {
	return backendOriginRequest(fetch, `/api/config-sessions/by-token/${token}`);
}

async function loadMetricAlerts(fetch: typeof globalThis.fetch) {
	try {
		const response = await backendOriginRequest(fetch, '/api/metric-alerts');
		if (!response.ok) {
			return { presets: [], rules: [] } satisfies MetricAlertsResponse;
		}
		return (await response.json()) as MetricAlertsResponse;
	} catch {
		return { presets: [], rules: [] } satisfies MetricAlertsResponse;
	}
}

async function rememberBrowserSession(
	fetch: typeof globalThis.fetch,
	token: string,
	payload: TelegramConfigSessionResponse
) {
	const now = Math.floor(Date.now() / 1000);
	if (payload.session.expires_at - now > REMEMBER_RENEW_BEFORE_SECONDS) {
		return { token, payload };
	}

	const response = await backendOriginRequest(
		fetch,
		`/api/config-sessions/by-token/${token}/remember`,
		{ method: 'POST' }
	);
	if (!response.ok) {
		return { token, payload };
	}

	const remembered = (await response.json()) as RememberTelegramConfigSessionResponse;
	return {
		token: remembered.token,
		payload: {
			...payload,
			session: remembered.session
		}
	};
}

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
		redirect(303, '/config');
	}

	const response = await loadConfigSession(fetch, token);
	if (!response.ok) {
		const cookieToken = rememberedToken(cookies);
		if (cookieToken && cookieToken !== token) {
			const cookieResponse = await loadConfigSession(fetch, cookieToken);
			if (cookieResponse.ok) {
				redirect(303, `/config/${cookieToken}`);
			}
			forgetConfigCookie(cookies);
		} else if (cookieToken === token) {
			forgetConfigCookie(cookies);
		}

		return {
			error: await readBackendError(response),
			session: null,
			user: null,
			billing: null,
			subscriptions: [],
			metricAlertPresets: [],
			metricAlertRules: [],
			customPlanPricing: customPlanPricing()
		};
	}

	const payload = (await response.json()) as TelegramConfigSessionResponse;
	const metricAlerts = await loadMetricAlerts(fetch);
	const remembered = await rememberBrowserSession(fetch, token, payload);
	rememberConfigCookie(cookies, remembered.token, remembered.payload.session.expires_at);

	return {
		error: null,
		session: remembered.payload.session,
		user: remembered.payload.user,
		billing: remembered.payload.billing,
		subscriptions: remembered.payload.subscriptions,
		metricAlertPresets: metricAlerts.presets,
		metricAlertRules: metricAlerts.rules,
		customPlanPricing: customPlanPricing()
	};
}) satisfies PageServerLoad;

function metricFieldName(key: string, field: string) {
	return `metric_rule:${field}:${key}`;
}

function metricField(form: FormData, key: string, field: string) {
	return String(form.get(metricFieldName(key, field)) ?? '').trim();
}

export const actions = {
	save_config: async ({ request, fetch, params, cookies }) => {
		const token = actionToken(cookies, params.token);
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
		const deliveryMode = String(form.get('delivery_mode') ?? 'live');
		const deliveryModes = form.getAll('delivery_modes').map(String);
		const clearingOnly =
			String(form.get('clear_config') ?? '') === 'true' &&
			form.getAll('protocol_slugs').length === 0 &&
			form.getAll('protocol_program_ids').length === 0 &&
			form.getAll('program_ids').length === 0 &&
			walletAddresses.length === 0 &&
			solDomains.length === 0;

		const response = await backendOriginRequest(
			fetch,
			`/api/config-sessions/by-token/${token}/save`,
			{
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({
					protocol_slugs: form.getAll('protocol_slugs').map(String),
					protocol_levels: form.getAll('protocol_levels').map(String),
					protocol_program_ids: form.getAll('protocol_program_ids').map(String),
					program_ids: form.getAll('program_ids').map(String),
					wallet_addresses: walletAddresses,
					sol_domains: solDomains,
					unsubscribe_ids: form.getAll('unsubscribe_ids').map(String),
					delivery_modes: deliveryModes.length > 0 ? deliveryModes : [deliveryMode],
					delivery_mode: deliveryMode,
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

		if (clearingOnly) {
			return {
				message:
					payload.saved_count === 0
						? 'Configuration already clear.'
						: `${payload.saved_count} watch ${payload.saved_count === 1 ? 'rule' : 'rules'} cleared.`
			};
		}

		return {
			message: `${payload.saved_count} watch ${payload.saved_count === 1 ? 'rule' : 'rules'} saved.`
		};
	},
	create_metric_alert_rule: async ({ request, fetch }) => {
		const form = await request.formData();
		const key = String(form.get('metric_rule_key') ?? '').trim();
		if (!key) {
			return fail(400, {
				error: 'Choose a metric trigger to save.'
			});
		}

		const label = metricField(form, key, 'label');
		const protocolSlug = metricField(form, key, 'protocol_slug');
		const instructionName = metricField(form, key, 'instruction_name');
		const metricKey = metricField(form, key, 'metric_key');
		const subjectAddress = metricField(form, key, 'subject_address');
		const minRaw = metricField(form, key, 'min_value');
		const maxRaw = metricField(form, key, 'max_value');
		const minValue = minRaw ? Number(minRaw) : null;
		const maxValue = maxRaw ? Number(maxRaw) : null;

		if (!label || !protocolSlug || !instructionName || !metricKey) {
			return fail(400, {
				error: 'Metric rule label, protocol, instruction, and metric are required.'
			});
		}

		if ((minRaw && Number.isNaN(minValue)) || (maxRaw && Number.isNaN(maxValue))) {
			return fail(400, {
				error: 'Metric thresholds must be numeric when provided.'
			});
		}

		if (minValue === null && maxValue === null) {
			return fail(400, {
				error: 'Set at least one metric threshold.'
			});
		}

		const response = await backendOriginRequest(fetch, '/api/metric-alert-rules', {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify({
				label,
				protocol_slug: protocolSlug,
				instruction_name: instructionName,
				metric_key: metricKey,
				subject_address: subjectAddress || null,
				min_value: minValue,
				max_value: maxValue
			})
		});

		if (!response.ok) {
			return fail(response.status, {
				error: await readBackendError(response)
			});
		}

		return {
			message: 'Metric trigger saved.'
		};
	},
	create_checkout: async ({ request, fetch, params, cookies }) => {
		const token = actionToken(cookies, params.token);
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
	},
	forget_config: async ({ cookies }) => {
		forgetConfigCookie(cookies);
		redirect(303, '/config');
	}
} satisfies Actions;
