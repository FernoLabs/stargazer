import { env as privateEnv } from '$env/dynamic/private';

export type ContactTelegramConfig = {
	apiBase: string;
	botToken: string;
	chatId: string;
	threadId: string;
	mention: string;
	webhookSecret: string;
};

const DEFAULT_TELEGRAM_API_BASE = 'https://api.telegram.org';
const TELEGRAM_REQUEST_TIMEOUT_MS = 8_000;

export function envValue(...keys: string[]) {
	for (const key of keys) {
		const value = privateEnv[key];
		if (value?.trim()) return value.trim();
	}

	return '';
}

export function contactTelegramConfig(): ContactTelegramConfig {
	return {
		apiBase: envValue('CONTACT_TELEGRAM_API_BASE', 'TELEGRAM_API_BASE') || DEFAULT_TELEGRAM_API_BASE,
		botToken: envValue('CONTACT_TELEGRAM_BOT_TOKEN', 'TELEGRAM_CONTACT_BOT_TOKEN'),
		chatId: envValue('CONTACT_TELEGRAM_CHAT_ID', 'TELEGRAM_CONTACT_CHAT_ID'),
		threadId: envValue('CONTACT_TELEGRAM_THREAD_ID', 'TELEGRAM_CONTACT_THREAD_ID'),
		mention: envValue('CONTACT_TELEGRAM_MENTION', 'TELEGRAM_CONTACT_MENTION') || '@AoCoH',
		webhookSecret: envValue('CONTACT_TELEGRAM_WEBHOOK_SECRET', 'TELEGRAM_CONTACT_WEBHOOK_SECRET')
	};
}

export function configuredContactChatId() {
	return contactTelegramConfig().chatId;
}

export async function contactTelegramRequest<T>(
	fetcher: typeof fetch,
	method: string,
	body: Record<string, unknown>
) {
	const config = contactTelegramConfig();
	if (!config.botToken) {
		throw new Error('Contact Telegram bot token is not configured.');
	}

	const controller = new AbortController();
	const timeout = setTimeout(() => controller.abort(), TELEGRAM_REQUEST_TIMEOUT_MS);

	try {
		const response = await fetcher(
			`${config.apiBase.replace(/\/+$/, '')}/bot${config.botToken}/${method}`,
			{
				method: 'POST',
				headers: {
					'content-type': 'application/json'
				},
				body: JSON.stringify(body),
				signal: controller.signal
			}
		);
		const responseText = await response.text();
		let payload: unknown = null;

		try {
			payload = responseText ? JSON.parse(responseText) : null;
		} catch {
			payload = null;
		}

		if (!response.ok || !telegramPayloadOk(payload)) {
			console.error('Telegram contact request failed', method, response.status, responseText);
			throw new Error(`Telegram ${method} failed.`);
		}

		return payload as T;
	} finally {
		clearTimeout(timeout);
	}
}

function telegramPayloadOk(payload: unknown) {
	if (!payload || typeof payload !== 'object') return false;
	const ok = (payload as { ok?: unknown }).ok;
	return ok === true;
}
