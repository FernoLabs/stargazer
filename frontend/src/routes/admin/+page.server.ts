import { fail } from '@sveltejs/kit';
import type {
	AdminUsersResponse,
	MetricAlertsResponse,
	PaymentLinksResponse,
	PromoCodesResponse,
	TelegramSubscriptionsResponse
} from '$lib/types';
import { backendOriginRequest, readBackendError } from '$lib/server/backend';
import type { Actions, PageServerLoad } from './$types';

export const load = (async ({ fetch }) => {
	let metricAlertPresets: MetricAlertsResponse['presets'] = [];
	let metricAlertRules: MetricAlertsResponse['rules'] = [];
	let firewatchUsers: AdminUsersResponse['users'] = [];
	let userEntitlements: AdminUsersResponse['entitlements'] = [];
	let paymentLinks: PaymentLinksResponse['items'] = [];
	let promoCodes: PromoCodesResponse['items'] = [];
	let telegramSubscriptions: TelegramSubscriptionsResponse['items'] = [];
	let telegramEnabled = false;
	let adminBackendError: string | null = null;

	try {
		const [metricAlertsResponse, usersResponse, promoCodesResponse, telegramResponse] = await Promise.all([
			backendOriginRequest(fetch, '/api/metric-alerts'),
			backendOriginRequest(fetch, '/api/users'),
			backendOriginRequest(fetch, '/api/promo-codes'),
			backendOriginRequest(fetch, '/api/telegram-subscriptions')
		]);

		if (!metricAlertsResponse.ok) {
			throw new Error(await readBackendError(metricAlertsResponse));
		}
		if (!usersResponse.ok) {
			throw new Error(await readBackendError(usersResponse));
		}
		if (!promoCodesResponse.ok) {
			throw new Error(await readBackendError(promoCodesResponse));
		}
		if (!telegramResponse.ok) {
			throw new Error(await readBackendError(telegramResponse));
		}

		const metricAlertsPayload = (await metricAlertsResponse.json()) as MetricAlertsResponse;
		metricAlertPresets = metricAlertsPayload.presets;
		metricAlertRules = metricAlertsPayload.rules;

		const usersPayload = (await usersResponse.json()) as AdminUsersResponse;
		firewatchUsers = usersPayload.users;
		userEntitlements = usersPayload.entitlements;
		paymentLinks = usersPayload.payment_links;

		const promoCodesPayload = (await promoCodesResponse.json()) as PromoCodesResponse;
		promoCodes = promoCodesPayload.items;

		const telegramPayload = (await telegramResponse.json()) as TelegramSubscriptionsResponse;
		telegramSubscriptions = telegramPayload.items;
		telegramEnabled = telegramPayload.telegram_enabled;
	} catch (error) {
		adminBackendError = error instanceof Error ? error.message : 'Unknown backend error';
	}

	return {
		metricAlertPresets,
		metricAlertRules,
		firewatchUsers,
		userEntitlements,
		paymentLinks,
		promoCodes,
		telegramSubscriptions,
		telegramEnabled,
		adminBackendError
	};
}) satisfies PageServerLoad;

export const actions = {
	create_telegram_subscription: async ({ request, fetch }) => {
		const form = await request.formData();
		const label = String(form.get('telegram_label') ?? '').trim();
		const chatId = String(form.get('telegram_chat_id') ?? '').trim();
		const threadIdRaw = String(form.get('telegram_thread_id') ?? '').trim();
		const scope = String(form.get('telegram_scope') ?? '').trim();
		const targetAddress = String(form.get('telegram_target_address') ?? '').trim();
		const requiredTags = form
			.getAll('telegram_required_tags')
			.map((value) => String(value).trim())
			.filter(Boolean);

		if (!label || !chatId) {
			return fail(400, {
				error: 'Telegram label and chat id are required.'
			});
		}

		if (scope !== 'program_updates' && scope !== 'multisig_activity' && scope !== 'defi_watch') {
			return fail(400, {
				error: 'Telegram scope must be program updates, multisig activity, or DeFi watch.'
			});
		}

		const threadId = threadIdRaw ? Number(threadIdRaw) : null;
		if (threadIdRaw && !Number.isInteger(threadId)) {
			return fail(400, {
				error: 'Telegram thread id must be an integer when provided.'
			});
		}

		const response = await backendOriginRequest(fetch, '/api/telegram-subscriptions', {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify({
				label,
				chat_id: chatId,
				thread_id: threadId,
				scope,
				target_address: targetAddress || null,
				required_tags: requiredTags
			})
		});

		if (!response.ok) {
			return fail(response.status, {
				error: await readBackendError(response)
			});
		}

		return {
			message: 'Telegram rule saved.'
		};
	},
	create_metric_alert_rule: async ({ request, fetch }) => {
		const form = await request.formData();
		const label = String(form.get('metric_rule_label') ?? '').trim();
		const protocolSlug = String(form.get('metric_protocol_slug') ?? '').trim();
		const instructionName = String(form.get('metric_instruction_name') ?? '').trim();
		const metricKey = String(form.get('metric_key') ?? '').trim();
		const subjectAddress = String(form.get('metric_subject_address') ?? '').trim();
		const minRaw = String(form.get('metric_min_value') ?? '').trim();
		const maxRaw = String(form.get('metric_max_value') ?? '').trim();
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
			message: 'Metric rule saved.'
		};
	},
	create_promo_code: async ({ request, fetch }) => {
		const form = await request.formData();
		const code = String(form.get('promo_code') ?? '').trim();
		const label = String(form.get('promo_label') ?? '').trim();
		const audience = String(form.get('promo_audience') ?? '').trim();
		const planId = String(form.get('promo_plan_id') ?? '').trim();
		const durationMonthsRaw = String(form.get('promo_duration_months') ?? '').trim();
		const maxRedemptionsRaw = String(form.get('promo_max_redemptions') ?? '').trim();
		const durationMonths = Number(durationMonthsRaw);
		const maxRedemptions = maxRedemptionsRaw ? Number(maxRedemptionsRaw) : null;

		if (!label || !audience || !planId) {
			return fail(400, {
				error: 'Promo label, audience, and plan are required.'
			});
		}

		if (!Number.isInteger(durationMonths) || durationMonths < 1) {
			return fail(400, {
				error: 'Promo duration must be a positive number of months.'
			});
		}

		if (maxRedemptionsRaw && (!Number.isInteger(maxRedemptions) || maxRedemptions === null || maxRedemptions < 1)) {
			return fail(400, {
				error: 'Promo max redemptions must be a positive integer when provided.'
			});
		}

		const response = await backendOriginRequest(fetch, '/api/promo-codes', {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify({
				code: code || null,
				label,
				audience,
				plan_id: planId,
				duration_months: durationMonths,
				max_redemptions: maxRedemptions,
				starts_at: null,
				expires_at: null
			})
		});

		if (!response.ok) {
			return fail(response.status, {
				error: await readBackendError(response)
			});
		}

		return {
			message: 'Promo code created.'
		};
	},
	create_user: async ({ request, fetch }) => {
		const form = await request.formData();
		const telegramUsername = String(form.get('user_telegram_username') ?? '').trim();
		const telegramUserIdRaw = String(form.get('user_telegram_user_id') ?? '').trim();
		const label = String(form.get('user_label') ?? '').trim();
		const telegramUserId = telegramUserIdRaw ? Number(telegramUserIdRaw) : null;

		if (!telegramUsername && !telegramUserIdRaw && !label) {
			return fail(400, {
				error: 'Add a Telegram handle, Telegram user id, or label.'
			});
		}

		if (telegramUserIdRaw && (!Number.isInteger(telegramUserId) || telegramUserId === null)) {
			return fail(400, {
				error: 'Telegram user id must be an integer.'
			});
		}

		const response = await backendOriginRequest(fetch, '/api/users', {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify({
				telegram_user_id: telegramUserId,
				telegram_username: telegramUsername || null,
				label: label || null
			})
		});

		if (!response.ok) {
			return fail(response.status, {
				error: await readBackendError(response)
			});
		}

		return {
			message: 'User saved.'
		};
	},
	grant_user_entitlement: async ({ request, fetch }) => {
		const form = await request.formData();
		const userId = String(form.get('entitlement_user_id') ?? '').trim();
		const planId = String(form.get('entitlement_plan_id') ?? '').trim();
		const source = String(form.get('entitlement_source') ?? '').trim();
		const durationMonthsRaw = String(form.get('entitlement_duration_months') ?? '').trim();
		const durationMonths = durationMonthsRaw ? Number(durationMonthsRaw) : null;

		if (!userId || !planId || !source) {
			return fail(400, {
				error: 'User, plan, and source are required.'
			});
		}

		if (durationMonthsRaw && (!Number.isInteger(durationMonths) || durationMonths === null || durationMonths < 1)) {
			return fail(400, {
				error: 'Entitlement duration must be a positive integer.'
			});
		}

		const response = await backendOriginRequest(fetch, '/api/users/entitlements', {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify({
				user_id: userId,
				plan_id: planId,
				source,
				duration_months: durationMonths,
				expires_at: null
			})
		});

		if (!response.ok) {
			return fail(response.status, {
				error: await readBackendError(response)
			});
		}

		return {
			message: 'User entitlement activated.'
		};
	},
	create_payment_link: async ({ request, fetch }) => {
		const form = await request.formData();
		const userId = String(form.get('payment_user_id') ?? '').trim();
		const amount = String(form.get('payment_amount') ?? '').trim();

		if (!userId) {
			return fail(400, {
				error: 'Select a user for the payment link.'
			});
		}

		const response = await backendOriginRequest(fetch, '/api/payment-links', {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify({
				user_id: userId,
				plan_id: 'paid',
				amount: amount || null
			})
		});

		if (!response.ok) {
			return fail(response.status, {
				error: await readBackendError(response)
			});
		}

		return {
			message: 'Solana Pay link generated.'
		};
	},
	delete_telegram_subscription: async ({ request, fetch }) => {
		const form = await request.formData();
		const id = String(form.get('id') ?? '').trim();

		if (!id) {
			return fail(400, {
				error: 'Missing Telegram subscription id.'
			});
		}

		const response = await backendOriginRequest(fetch, `/api/telegram-subscriptions/${id}`, {
			method: 'DELETE'
		});

		if (!response.ok) {
			return fail(response.status, {
				error: await readBackendError(response)
			});
		}

		return {
			message: 'Telegram rule removed.'
		};
	},
	delete_promo_code: async ({ request, fetch }) => {
		const form = await request.formData();
		const id = String(form.get('id') ?? '').trim();

		if (!id) {
			return fail(400, {
				error: 'Missing promo code id.'
			});
		}

		const response = await backendOriginRequest(fetch, `/api/promo-codes/${id}`, {
			method: 'DELETE'
		});

		if (!response.ok) {
			return fail(response.status, {
				error: await readBackendError(response)
			});
		}

		return {
			message: 'Promo code removed.'
		};
	},
	delete_metric_alert_rule: async ({ request, fetch }) => {
		const form = await request.formData();
		const id = String(form.get('id') ?? '').trim();

		if (!id) {
			return fail(400, {
				error: 'Missing metric alert rule id.'
			});
		}

		const response = await backendOriginRequest(fetch, `/api/metric-alert-rules/${id}`, {
			method: 'DELETE'
		});

		if (!response.ok) {
			return fail(response.status, {
				error: await readBackendError(response)
			});
		}

		return {
			message: 'Metric rule removed.'
		};
	}
} satisfies Actions;
