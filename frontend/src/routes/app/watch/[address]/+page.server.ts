import { fail } from '@sveltejs/kit';
import { parseStellarWalletAddress } from '$lib/dashboard/stellar';
import { createAlertControlClient, alertControlErrorMessage } from '$lib/server/alert-control';
import { setAlertPairingCookie } from '$lib/server/alert-pairing-cookie';
import { createLiveDashboardClient, liveDashboardErrorMessage } from '$lib/server/live-dashboard';
import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, params, platform, setHeaders }) => {
	setHeaders({
		'cache-control': 'private, no-store',
		'referrer-policy': 'no-referrer'
	});

	const parsed = parseStellarWalletAddress(params.address);
	if (!parsed.ok) {
		return {
			kind: 'invalid' as const,
			input: params.address,
			message: parsed.error.message
		};
	}

	try {
		return {
			kind: 'ready' as const,
			walletKind: parsed.value.kind,
			dashboard: await createLiveDashboardClient(
				fetch,
				platform?.env.STARGAZER_NAS_GATEWAY
			).getWallet(parsed.value.address, { limit: 1 })
		};
	} catch (error) {
		return {
			kind: 'unavailable' as const,
			address: parsed.value.address,
			message: liveDashboardErrorMessage(error)
		};
	}
};

export const actions = {
	default: async ({ cookies, fetch, params, request, url }) => {
		const parsed = parseStellarWalletAddress(params.address);
		if (!parsed.ok) return fail(400, { message: parsed.error.message });

		const values = await request.formData();
		const criticalAlerts = values.get('criticalAlerts') === 'on';
		const nonCriticalAlerts = values.get('nonCriticalAlerts') === 'on';
		if (!criticalAlerts && !nonCriticalAlerts) {
			return fail(400, {
				message: 'Choose critical alerts, important updates, or both.',
				criticalAlerts,
				nonCriticalAlerts
			});
		}

		try {
			const pairing = await createAlertControlClient(fetch).createPairing({
				walletAddress: parsed.value.accountAddress,
				criticalAlerts,
				nonCriticalAlerts
			});
			setAlertPairingCookie(
				cookies,
				pairing.claimToken,
				pairing.expiresAt,
				url.protocol === 'https:'
			);
			return {
				success: true as const,
				criticalAlerts,
				nonCriticalAlerts,
				pairing: {
					telegramUrl: pairing.telegramUrl,
					statusUrl: `${url.pathname.replace(/\/$/, '')}/status`,
					expiresAt: new Date(pairing.expiresAt * 1000).toISOString(),
					status: pairing.status,
					billing: pairing.billing
				}
			};
		} catch (error) {
			return fail(503, {
				message: alertControlErrorMessage(error),
				criticalAlerts,
				nonCriticalAlerts
			});
		}
	}
} satisfies Actions;
