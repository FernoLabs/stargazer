import {
	DEFAULT_EXPLORER_CLUSTER,
	loadAppShell,
	publicBackendBaseUrl,
	type AppShellLoadData
} from '$lib/server/backend';
import { customPlanPricing } from '$lib/server/custom-plan-pricing';
import { isPulsarRoute } from '$lib/pulsar-routes';
import type { LayoutServerLoad } from './$types';

function landingShell(): AppShellLoadData {
	return {
		backendBaseUrl: publicBackendBaseUrl(),
		backendError: null,
		explorerCluster: DEFAULT_EXPLORER_CLUSTER,
		liveStatus: null,
		defiProtocols: [],
		telegramEnabled: false,
		telegramSubscriptionCount: 0,
		protocolCount: 0,
		trackedProgramCount: 0,
		idlTrackedProgramCount: 0,
		deepDecodeProtocolCount: 0,
		customPlanPricing: customPlanPricing()
	};
}

export const load = (async ({ fetch, url }) => {
	if (isPulsarRoute(url.pathname)) return landingShell();
	return loadAppShell(fetch);
}) satisfies LayoutServerLoad;
