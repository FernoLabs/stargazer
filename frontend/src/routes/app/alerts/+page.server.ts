import {
	buildAlertRegister,
	type AlertRegisterCategory
} from '$lib/dashboard/alert-register';
import { createLiveDashboardClient, liveDashboardErrorMessage } from '$lib/server/live-dashboard';
import type { PageServerLoad } from './$types';

type CategoryFilter = 'all' | AlertRegisterCategory;

export const load: PageServerLoad = async ({ fetch, platform, url }) => {
	const filters = {
		category: parseCategory(url.searchParams.get('category'))
	};

	let catalog = null;
	let archiveMessage: string | null = null;
	try {
		catalog = await createLiveDashboardClient(
			fetch,
			platform?.env.STARGAZER_NAS_GATEWAY
		).getProtocols();
	} catch (error) {
		archiveMessage = liveDashboardErrorMessage(error);
	}

	const allRecords = buildAlertRegister(catalog);
	const records = allRecords.filter(
		(record) => filters.category === 'all' || record.category === filters.category
	);

	return {
		filters,
		records,
		totals: {
			incidents: allRecords.filter((record) => record.category === 'incident').length,
			upgrades: catalog
				? allRecords.filter((record) => record.category === 'contract-upgrade').length
				: null,
			controlChanges: catalog
				? allRecords.filter((record) => record.category === 'control-change').length
				: null,
			critical: allRecords.filter((record) => record.severity === 'critical').length
		},
		archive: catalog
			? {
					available: true as const,
					source: catalog.source,
					governance: catalog.governance
				}
			: {
					available: false as const,
					message: archiveMessage ?? 'The archive service is unavailable.'
				}
	};
};

function parseCategory(value: string | null): CategoryFilter {
	return value === 'incident' || value === 'contract-upgrade' || value === 'control-change'
		? value
		: 'all';
}
