import type { Cookies } from '@sveltejs/kit';

export const ALERT_PAIRING_COOKIE = 'stargazer_alert_pairing';
export const ALERT_PAIRING_COOKIE_PATH = '/app/watch';

export function setAlertPairingCookie(
	cookies: Cookies,
	claimToken: string,
	expiresAt: number,
	secure: boolean
): void {
	const now = Math.floor(Date.now() / 1000);
	cookies.set(ALERT_PAIRING_COOKIE, claimToken, {
		path: ALERT_PAIRING_COOKIE_PATH,
		httpOnly: true,
		sameSite: 'strict',
		secure,
		maxAge: Math.max(1, expiresAt - now)
	});
}

export function clearAlertPairingCookie(cookies: Cookies, secure: boolean): void {
	cookies.delete(ALERT_PAIRING_COOKIE, {
		path: ALERT_PAIRING_COOKIE_PATH,
		httpOnly: true,
		sameSite: 'strict',
		secure
	});
}
