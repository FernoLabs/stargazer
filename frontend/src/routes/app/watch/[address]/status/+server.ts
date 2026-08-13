import { error, json } from '@sveltejs/kit';
import { AlertControlError } from '$lib/alerts/control';
import { parseStellarWalletAddress } from '$lib/dashboard/stellar';
import { createAlertControlClient } from '$lib/server/alert-control';
import {
	ALERT_PAIRING_COOKIE,
	clearAlertPairingCookie
} from '$lib/server/alert-pairing-cookie';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ cookies, fetch, params, setHeaders, url }) => {
	setHeaders({
		'cache-control': 'private, no-store, max-age=0',
		'referrer-policy': 'no-referrer'
	});

	const wallet = parseStellarWalletAddress(params.address);
	if (!wallet.ok) error(400, 'This Stellar wallet address is invalid.');

	const claimToken = cookies.get(ALERT_PAIRING_COOKIE);
	if (!claimToken) error(401, 'No active Telegram setup was found in this browser.');

	try {
		const status = await createAlertControlClient(fetch).getPairingStatus(claimToken);
		if (status.walletAddress !== wallet.value.accountAddress) {
			clearAlertPairingCookie(cookies, url.protocol === 'https:');
			error(409, 'This Telegram setup belongs to another wallet.');
		}
		if (status.status === 'expired') {
			clearAlertPairingCookie(cookies, url.protocol === 'https:');
		}
		return json(status);
	} catch (cause) {
		if (
			cause instanceof AlertControlError &&
			(cause.status === 401 || cause.status === 404 || cause.code === 'invalid_claim_token')
		) {
			clearAlertPairingCookie(cookies, url.protocol === 'https:');
			error(410, 'This Telegram setup link is no longer available.');
		}
		throw cause;
	}
};
