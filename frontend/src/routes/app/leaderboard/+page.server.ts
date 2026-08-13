import { isLeaderboardUnavailable } from '$lib/dashboard/live';
import type { LeaderboardMetric } from '$lib/dashboard/types';
import { createLiveDashboardClient, liveDashboardErrorMessage } from '$lib/server/live-dashboard';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, platform, url }) => {
	const requestedMetric = url.searchParams.get('sort');
	const metric: LeaderboardMetric = requestedMetric === 'protocols' ? 'protocols' : 'transactions';

	try {
		return {
			kind: 'ready' as const,
			leaderboard: await createLiveDashboardClient(
				fetch,
				platform?.env.STARGAZER_NAS_GATEWAY
			).getLeaderboard({ metric, limit: 50 })
		};
	} catch (error) {
		if (isLeaderboardUnavailable(error)) {
			return {
				kind: 'building' as const,
				metric,
				message: error.message
			};
		}

		return {
			kind: 'unavailable' as const,
			metric,
			message: liveDashboardErrorMessage(error)
		};
	}
};
