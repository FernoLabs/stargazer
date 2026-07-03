import { env as privateEnv } from '$env/dynamic/private';
import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';

type ContactPayload = {
	name?: unknown;
	company?: unknown;
	telegram?: unknown;
	email?: unknown;
	message?: unknown;
	source?: unknown;
	website?: unknown;
};

type ContactRequest = {
	name: string;
	company: string;
	telegram: string;
	email: string;
	message: string;
	source: string;
};

const TELEGRAM_API_BASE = 'https://api.telegram.org';
const SEND_TIMEOUT_MS = 8_000;
const MAX_MESSAGE_LENGTH = 2_000;
const MAX_TEXT_LENGTH = 3_800;

function jsonHeaders() {
	return {
		'cache-control': 'no-store'
	};
}

function stringValue(value: unknown) {
	return typeof value === 'string' ? value.trim() : '';
}

function singleLine(value: unknown, maxLength: number) {
	return stringValue(value).replace(/\s+/g, ' ').slice(0, maxLength);
}

function multiline(value: unknown, maxLength: number) {
	return stringValue(value).replace(/\r\n/g, '\n').slice(0, maxLength);
}

function emailLooksValid(value: string) {
	return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value);
}

function normalizedTelegram(value: unknown) {
	const handle = singleLine(value, 80).replace(/^https?:\/\/t\.me\//i, '@');
	if (!handle) return '';
	return handle.startsWith('@') ? handle : `@${handle}`;
}

function envValue(...keys: string[]) {
	for (const key of keys) {
		const value = privateEnv[key];
		if (value?.trim()) return value.trim();
	}

	return '';
}

function validatePayload(payload: ContactPayload): ContactRequest | Response {
	const name = singleLine(payload.name, 120);
	const company = singleLine(payload.company, 140);
	const telegram = normalizedTelegram(payload.telegram);
	const email = singleLine(payload.email, 160).toLowerCase();
	const message = multiline(payload.message, MAX_MESSAGE_LENGTH);
	const source = singleLine(payload.source, 160);

	if (!name) {
		return json({ error: 'Tell us your name.' }, { status: 400, headers: jsonHeaders() });
	}

	if (!telegram && !email) {
		return json(
			{ error: 'Add a Telegram handle or an email so we can reply.' },
			{ status: 400, headers: jsonHeaders() }
		);
	}

	if (email && !emailLooksValid(email)) {
		return json({ error: 'Use a valid email address.' }, { status: 400, headers: jsonHeaders() });
	}

	if (!message) {
		return json(
			{ error: 'Tell us what you want to monitor.' },
			{ status: 400, headers: jsonHeaders() }
		);
	}

	return { name, company, telegram, email, message, source };
}

function truncateTelegramText(text: string) {
	return text.length <= MAX_TEXT_LENGTH ? text : `${text.slice(0, MAX_TEXT_LENGTH - 2)}..`;
}

function formatContactMessage(contact: ContactRequest) {
	const mention = envValue('CONTACT_TELEGRAM_MENTION', 'TELEGRAM_CONTACT_MENTION') || '@AoCoH';
	const lines = [
		`${mention} New FireWatch contact request`,
		'',
		`Name: ${contact.name}`,
		contact.company ? `Company: ${contact.company}` : null,
		contact.telegram ? `Telegram: ${contact.telegram}` : null,
		contact.email ? `Email: ${contact.email}` : null,
		contact.source ? `Source: ${contact.source}` : null,
		'',
		'Request:',
		contact.message
	].filter((line): line is string => line != null);

	return truncateTelegramText(lines.join('\n'));
}

async function parseRequest(request: Request) {
	try {
		return (await request.json()) as ContactPayload;
	} catch {
		return null;
	}
}

async function sendTelegramMessage(fetcher: typeof fetch, text: string) {
	const botToken = envValue('CONTACT_TELEGRAM_BOT_TOKEN', 'TELEGRAM_CONTACT_BOT_TOKEN');
	const chatId = envValue('CONTACT_TELEGRAM_CHAT_ID', 'TELEGRAM_CONTACT_CHAT_ID');
	const threadId = envValue('CONTACT_TELEGRAM_THREAD_ID', 'TELEGRAM_CONTACT_THREAD_ID');
	const apiBase = envValue('CONTACT_TELEGRAM_API_BASE', 'TELEGRAM_API_BASE') || TELEGRAM_API_BASE;

	if (!botToken || !chatId) {
		return json(
			{ error: 'Contact form is not configured yet.' },
			{ status: 503, headers: jsonHeaders() }
		);
	}

	const body: Record<string, string | number | boolean> = {
		chat_id: chatId,
		text,
		disable_web_page_preview: true
	};
	const parsedThreadId = Number.parseInt(threadId, 10);
	if (Number.isFinite(parsedThreadId) && parsedThreadId > 0) {
		body.message_thread_id = parsedThreadId;
	}

	const controller = new AbortController();
	const timeout = setTimeout(() => controller.abort(), SEND_TIMEOUT_MS);

	try {
		const response = await fetcher(`${apiBase.replace(/\/+$/, '')}/bot${botToken}/sendMessage`, {
			method: 'POST',
			headers: {
				'content-type': 'application/json'
			},
			body: JSON.stringify(body),
			signal: controller.signal
		});

		if (!response.ok) {
			console.error('Telegram contact send failed', response.status, await response.text());
			return json(
				{ error: 'Could not send the message. Please try again.' },
				{ status: 502, headers: jsonHeaders() }
			);
		}

		return json({ ok: true }, { headers: jsonHeaders() });
	} catch (error) {
		console.error('Telegram contact send errored', error);
		return json(
			{ error: 'Could not send the message. Please try again.' },
			{ status: 502, headers: jsonHeaders() }
		);
	} finally {
		clearTimeout(timeout);
	}
}

export const POST = (async ({ request, fetch }) => {
	const payload = await parseRequest(request);
	if (!payload) {
		return json({ error: 'Invalid contact request.' }, { status: 400, headers: jsonHeaders() });
	}

	if (stringValue(payload.website)) {
		return json({ ok: true }, { headers: jsonHeaders() });
	}

	const contact = validatePayload(payload);
	if (contact instanceof Response) return contact;

	return sendTelegramMessage(fetch, formatContactMessage(contact));
}) satisfies RequestHandler;
