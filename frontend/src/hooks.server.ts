import { redirect, type Handle } from '@sveltejs/kit';

const CANONICAL_ORIGIN = 'https://stargazer.ferno.ag';

// The Cloudflare Pages project keeps its original name, so its pages.dev host serves the same site.
// Only the bare project host redirects; branch previews (<branch>.ferno-pulsar.pages.dev) keep working.
const RETIRED_HOSTS = new Set(['ferno-pulsar.pages.dev']);

export const handle: Handle = async ({ event, resolve }) => {
	if (RETIRED_HOSTS.has(event.url.hostname)) {
		redirect(301, `${CANONICAL_ORIGIN}${event.url.pathname}${event.url.search}`);
	}
	return resolve(event);
};
