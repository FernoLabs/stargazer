import { json, type RequestHandler } from '@sveltejs/kit';
import { backendOriginRequest, readBackendError } from '$lib/server/backend';
import type { WalletHistoryScanResponse } from '$lib/types';

type ScanWalletHistoryRequest = {
	wallet_addresses?: unknown;
	sol_domains?: unknown;
	limit?: unknown;
};

function stringList(value: unknown) {
	if (!Array.isArray(value)) return [];
	return value.map((item) => String(item).trim()).filter(Boolean);
}

export const POST: RequestHandler = async ({ fetch, params, request }) => {
	const token = params.token?.trim() ?? '';
	const payload = (await request.json()) as ScanWalletHistoryRequest;
	const response = await backendOriginRequest(
		fetch,
		`/api/config-sessions/by-token/${token}/scan-wallets`,
		{
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify({
				wallet_addresses: stringList(payload.wallet_addresses),
				sol_domains: stringList(payload.sol_domains),
				limit: typeof payload.limit === 'number' ? payload.limit : undefined
			})
		}
	);

	if (!response.ok) {
		return json({ error: await readBackendError(response) }, { status: response.status });
	}

	return json((await response.json()) as WalletHistoryScanResponse);
};
