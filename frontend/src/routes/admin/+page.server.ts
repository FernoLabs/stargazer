import { fail } from '@sveltejs/kit';
import type { MetricAlertsResponse } from '$lib/types';
import { backendRequest, readBackendError } from '$lib/server/backend';
import type { Actions, PageServerLoad } from './$types';

export const load = (async ({ fetch }) => {
	let metricAlertPresets: MetricAlertsResponse['presets'] = [];
	let metricAlertRules: MetricAlertsResponse['rules'] = [];
	let metricBackendError: string | null = null;

	try {
		const metricAlertsResponse = await backendRequest(fetch, '/api/metric-alerts');
		if (!metricAlertsResponse.ok) {
			throw new Error(await readBackendError(metricAlertsResponse));
		}

		const metricAlertsPayload = (await metricAlertsResponse.json()) as MetricAlertsResponse;
		metricAlertPresets = metricAlertsPayload.presets;
		metricAlertRules = metricAlertsPayload.rules;
	} catch (error) {
		metricBackendError = error instanceof Error ? error.message : 'Unknown backend error';
	}

	return {
		metricAlertPresets,
		metricAlertRules,
		metricBackendError
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

		const response = await backendRequest(fetch, '/api/telegram-subscriptions', {
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

		const response = await backendRequest(fetch, '/api/metric-alert-rules', {
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
	delete_telegram_subscription: async ({ request, fetch }) => {
		const form = await request.formData();
		const id = String(form.get('id') ?? '').trim();

		if (!id) {
			return fail(400, {
				error: 'Missing Telegram subscription id.'
			});
		}

		const response = await backendRequest(fetch, `/api/telegram-subscriptions/${id}`, {
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
	delete_metric_alert_rule: async ({ request, fetch }) => {
		const form = await request.formData();
		const id = String(form.get('id') ?? '').trim();

		if (!id) {
			return fail(400, {
				error: 'Missing metric alert rule id.'
			});
		}

		const response = await backendRequest(fetch, `/api/metric-alert-rules/${id}`, {
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
