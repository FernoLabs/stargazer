import { parseStellarWalletAddress } from '$lib/dashboard/stellar';
import { createLiveDashboardClient, liveDashboardErrorMessage } from '$lib/server/live-dashboard';
import type { StellarContractAddress } from '$lib/dashboard/types';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, params, platform, url }) => {
	const parsed = parseStellarWalletAddress(params.address);
	if (!parsed.ok) {
		return {
			kind: 'invalid' as const,
			input: params.address,
			message: parsed.error.message
		};
	}
	const cursor = parseContractCursor(url.searchParams.get('cursor'));

	try {
		return {
			kind: 'ready' as const,
			walletKind: parsed.value.kind,
			contractCursor: cursor,
			dashboard: await createLiveDashboardClient(
				fetch,
				platform?.env.STARGAZER_NAS_GATEWAY
			).getWallet(parsed.value.address, {
				limit: 500,
				...(cursor ? { cursor } : {})
			})
		};
	} catch (error) {
		return {
			kind: 'unavailable' as const,
			address: parsed.value.address,
			message: liveDashboardErrorMessage(error)
		};
	}
};

function parseContractCursor(value: string | null): StellarContractAddress | null {
	return value && /^C[A-Z2-7]{55}$/.test(value) ? (value as StellarContractAddress) : null;
}
