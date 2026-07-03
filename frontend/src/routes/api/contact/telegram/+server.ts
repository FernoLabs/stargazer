import { json } from '@sveltejs/kit';
import {
	configuredContactChatId,
	contactTelegramConfig,
	contactTelegramRequest
} from '$lib/server/contact-telegram';
import type { RequestHandler } from './$types';

type TelegramUser = {
	id: number;
	username?: string;
	first_name?: string;
	last_name?: string;
};

type TelegramChat = {
	id: number | string;
	title?: string;
	type?: string;
};

type TelegramMessage = {
	message_id: number;
	chat: TelegramChat;
	text?: string;
	from?: TelegramUser;
	reply_to_message?: TelegramMessage;
};

type TelegramCallbackQuery = {
	id: string;
	from: TelegramUser;
	message?: TelegramMessage;
	data?: string;
};

type TelegramUpdate = {
	update_id?: number;
	callback_query?: TelegramCallbackQuery;
	message?: TelegramMessage;
};

function noStoreHeaders() {
	return {
		'cache-control': 'no-store'
	};
}

async function parseUpdate(request: Request) {
	try {
		return (await request.json()) as TelegramUpdate;
	} catch {
		return null;
	}
}

function sameContactChat(chat: TelegramChat | undefined) {
	if (!chat) return false;
	return String(chat.id) === configuredContactChatId();
}

function userLabel(user: TelegramUser | undefined) {
	if (!user) return 'unknown group member';
	if (user.username?.trim()) return `@${user.username.trim()}`;
	const name = [user.first_name, user.last_name].filter(Boolean).join(' ').trim();
	return name || `Telegram user ${user.id}`;
}

function utcMinuteLabel(date = new Date()) {
	return date.toISOString().replace('T', ' ').slice(0, 16);
}

function isHandledText(text: string | undefined) {
	return Boolean(text && /\n\nStatus: Handled by /.test(text));
}

function handledMessageText(originalText: string, user: TelegramUser) {
	const baseText = originalText.replace(/\n\nStatus: (Open|Handled by[\s\S]*)$/, '').trimEnd();
	return `${baseText}\n\nStatus: Handled by ${userLabel(user)} at ${utcMinuteLabel()} UTC`;
}

async function answerCallbackQuery(fetcher: typeof fetch, id: string, text: string) {
	await contactTelegramRequest(fetcher, 'answerCallbackQuery', {
		callback_query_id: id,
		text,
		show_alert: false
	});
}

async function markHandled(fetcher: typeof fetch, message: TelegramMessage, user: TelegramUser) {
	if (!message.text) return false;

	await contactTelegramRequest(fetcher, 'editMessageText', {
		chat_id: message.chat.id,
		message_id: message.message_id,
		text: handledMessageText(message.text, user),
		disable_web_page_preview: true,
		reply_markup: {
			inline_keyboard: []
		}
	});

	return true;
}

async function handleCallback(fetcher: typeof fetch, callbackQuery: TelegramCallbackQuery) {
	if (callbackQuery.data !== 'contact:handled') {
		await answerCallbackQuery(fetcher, callbackQuery.id, 'Unknown action.');
		return;
	}

	if (!callbackQuery.message || !sameContactChat(callbackQuery.message.chat)) {
		await answerCallbackQuery(fetcher, callbackQuery.id, 'This request is not in the contact group.');
		return;
	}

	if (isHandledText(callbackQuery.message.text)) {
		await answerCallbackQuery(fetcher, callbackQuery.id, 'Already handled.');
		return;
	}

	try {
		const handled = await markHandled(fetcher, callbackQuery.message, callbackQuery.from);
		await answerCallbackQuery(
			fetcher,
			callbackQuery.id,
			handled ? `Handled by ${userLabel(callbackQuery.from)}.` : 'Could not read this request.'
		);
	} catch (error) {
		console.error('Failed to handle contact callback', error);
		await answerCallbackQuery(fetcher, callbackQuery.id, 'Could not mark handled.');
	}
}

function isHandledReply(message: TelegramMessage) {
	return /^\/?(handled|done)$/i.test((message.text ?? '').trim());
}

async function handleReply(fetcher: typeof fetch, message: TelegramMessage) {
	if (!sameContactChat(message.chat) || !message.reply_to_message || !isHandledReply(message)) return;
	if (isHandledText(message.reply_to_message.text)) return;

	try {
		await markHandled(fetcher, message.reply_to_message, message.from ?? { id: 0 });
	} catch (error) {
		console.error('Failed to handle contact reply', error);
	}
}

export const POST = (async ({ request, fetch }) => {
	const config = contactTelegramConfig();
	if (!config.webhookSecret) {
		return json({ error: 'Contact webhook is not configured.' }, { status: 503 });
	}

	if (request.headers.get('x-telegram-bot-api-secret-token') !== config.webhookSecret) {
		return json({ error: 'Unauthorized' }, { status: 401, headers: noStoreHeaders() });
	}

	const update = await parseUpdate(request);
	if (!update) {
		return json({ error: 'Invalid Telegram update.' }, { status: 400, headers: noStoreHeaders() });
	}

	if (update.callback_query) {
		await handleCallback(fetch, update.callback_query);
	} else if (update.message) {
		await handleReply(fetch, update.message);
	}

	return json({ ok: true }, { headers: noStoreHeaders() });
}) satisfies RequestHandler;
