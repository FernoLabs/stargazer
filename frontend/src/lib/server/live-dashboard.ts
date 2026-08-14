import { dev } from '$app/environment';
import { env } from '$env/dynamic/private';
import {
	LiveDashboardClient,
	LiveDashboardError,
	LiveDashboardHttpError
} from '$lib/dashboard/live';

const DEFAULT_API_BASE = 'http://127.0.0.1:8787';
const PRIVATE_GATEWAY_BASE = 'http://stargazer-private-gateway.invalid';
type PrivateGateway = NonNullable<App.Platform['env']['STARGAZER_NAS_GATEWAY']>;

export function createLiveDashboardClient(
	fetcher: typeof fetch,
	privateGateway?: PrivateGateway
): LiveDashboardClient {
	if (!dev && privateGateway) {
		const privateFetch: typeof fetch = (input, init) => privateGateway.fetch(input, init);
		return new LiveDashboardClient(PRIVATE_GATEWAY_BASE, { fetch: privateFetch });
	}

	const configuredBase = env.STARGAZER_API_BASE?.trim();
	return new LiveDashboardClient(configuredBase || DEFAULT_API_BASE, { fetch: fetcher });
}

export function liveDashboardErrorMessage(error: unknown): string {
	if (error instanceof LiveDashboardHttpError) return error.message;

	if (error instanceof LiveDashboardError) {
		switch (error.kind) {
			case 'timeout':
				return 'The archive query took too long. Try again in a moment.';
			case 'network':
				return 'The NAS read service cannot be reached right now.';
			case 'configuration':
				return 'The archive read service is not configured correctly.';
			case 'invalid-response':
				return 'The archive read service returned an unexpected response.';
			case 'aborted':
				return 'The archive query was cancelled.';
			case 'http':
				return 'The archive read service could not complete this query.';
		}
	}

	return 'The archive read service could not complete this query.';
}
