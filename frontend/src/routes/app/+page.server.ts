import { createLiveDashboardClient, liveDashboardErrorMessage } from '$lib/server/live-dashboard';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, platform }) => {
	try {
		const client = createLiveDashboardClient(fetch, platform?.env.STARGAZER_NAS_GATEWAY);
		const examples = await client.getExampleWallets({ limit: 5 });
		let featuredWallet = null;

		try {
			const activity = await client.getLeaderboard({ metric: 'protocols', limit: 10 });
			const candidate = activity.rows[0] ?? null;
			if (candidate) {
				const report = await client.getWallet(candidate.accountAddress, { limit: 1 });
				featuredWallet = {
					...candidate,
					alertCount: report.summary.alertCount
				};
			}
		} catch {
			// The wallet check remains useful while the optional activity snapshot is rebuilding.
		}

		return {
			kind: 'ready' as const,
			examples,
			featuredWallet
		};
	} catch (error) {
		return {
			kind: 'unavailable' as const,
			message: liveDashboardErrorMessage(error)
		};
	}
};
