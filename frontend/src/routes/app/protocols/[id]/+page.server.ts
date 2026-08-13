import { protocolDisplayName } from '$lib/dashboard/protocols';
import { createLiveDashboardClient, liveDashboardErrorMessage } from '$lib/server/live-dashboard';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, params, platform }) => {
	try {
		const catalog = await createLiveDashboardClient(
			fetch,
			platform?.env.STARGAZER_NAS_GATEWAY
		).getProtocols();
		const protocol = catalog.protocols.find((entry) => entry.protocolId === params.id);
		if (!protocol) {
			return {
				kind: 'missing' as const,
				protocolId: params.id
			};
		}

		const identities = new Set([
			canonicalProtocolIdentity(protocol.protocolId),
			canonicalProtocolIdentity(protocolDisplayName(protocol.protocolId))
		]);
		const controllers = catalog.governance.available
			? catalog.governance.controllers.filter((controller) =>
					identities.has(canonicalProtocolIdentity(controller.protocol))
				)
			: [];
		const controllerAccounts = new Set(controllers.map((controller) => controller.accountId));
		const governanceAlerts = catalog.governance.available
			? catalog.governance.alerts.filter((alert) => controllerAccounts.has(alert.accountId))
			: [];

		return {
			kind: 'ready' as const,
			protocol,
			source: catalog.source,
			controllers,
			governanceAlerts,
			governanceAvailable: catalog.governance.available,
			governanceReason: catalog.governance.reason
		};
	} catch (error) {
		return {
			kind: 'unavailable' as const,
			protocolId: params.id,
			message: liveDashboardErrorMessage(error)
		};
	}
};

function canonicalProtocolIdentity(value: string): string {
	return value.toLowerCase().replaceAll(/[^a-z0-9]/g, '');
}
