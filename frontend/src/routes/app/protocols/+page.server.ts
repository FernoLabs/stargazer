import type { StellarContractAddress } from '$lib/dashboard/types';
import { createLiveDashboardClient, liveDashboardErrorMessage } from '$lib/server/live-dashboard';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, platform, url }) => {
	const attribution = url.searchParams.get('attribution') === 'unattributed' ? 'unattributed' : 'verified';
	const cursor =
		attribution === 'unattributed'
			? parseContractCursor(url.searchParams.get('unattributedCursor'))
			: null;

	try {
		const client = createLiveDashboardClient(fetch, platform?.env.STARGAZER_NAS_GATEWAY);
		if (attribution === 'unattributed') {
			return {
				kind: 'ready' as const,
				attribution,
				catalog: null,
				unattributedContracts: await client.getUnattributedContracts({
					limit: 100,
					...(cursor ? { cursor } : {})
				}),
				unattributedCursor: cursor
			};
		}

		return {
			kind: 'ready' as const,
			attribution,
			catalog: await client.getProtocols(),
			unattributedContracts: null,
			unattributedCursor: null
		};
	} catch (error) {
		return {
			kind: 'unavailable' as const,
			message: liveDashboardErrorMessage(error),
			attribution
		};
	}
};

function parseContractCursor(value: string | null): StellarContractAddress | null {
	return value && /^C[A-Z2-7]{55}$/.test(value) ? (value as StellarContractAddress) : null;
}
