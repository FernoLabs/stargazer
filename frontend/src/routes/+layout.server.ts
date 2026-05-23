import { loadAppShell } from '$lib/server/backend';
import type { LayoutServerLoad } from './$types';

export const load = (async ({ fetch }) => {
	return loadAppShell(fetch);
}) satisfies LayoutServerLoad;
