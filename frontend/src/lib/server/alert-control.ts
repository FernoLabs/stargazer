import { env } from '$env/dynamic/private';
import { AlertControlClient, AlertControlError } from '$lib/alerts/control';

const DEFAULT_ALERT_CONTROL_BASE = 'http://127.0.0.1:8788';

export function createAlertControlClient(fetcher: typeof fetch): AlertControlClient {
	return new AlertControlClient(
		env.STARGAZER_ALERT_CONTROL_BASE?.trim() || DEFAULT_ALERT_CONTROL_BASE,
		fetcher
	);
}

export function alertControlErrorMessage(error: unknown): string {
	if (error instanceof AlertControlError) return error.message;
	return 'The Telegram setup service cannot be reached right now.';
}
