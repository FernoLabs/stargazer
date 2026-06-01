import { error, redirect } from '@sveltejs/kit';
import { syntheticSquadsDescriptor } from '$lib/catalog';
import type { DefiProtocolDescriptor } from '$lib/types';
import type { PageServerLoad } from './$types';

const FORWARDED_FILTERS = ['scope', 'search', 'severity', 'coverage', 'tag', 'target'] as const;

export const load = (async ({ params, parent, url }) => {
	const app = await parent();
	const slug = params.slug;
	const protocol =
		(app.defiProtocols.find((candidate) => candidate.slug === slug) as
			| DefiProtocolDescriptor
			| undefined) ?? (slug === 'squads' ? syntheticSquadsDescriptor() : undefined);

	if (!protocol) {
		throw error(404, 'Protocol not found');
	}

	const search = new URLSearchParams();
	search.set('protocol', protocol.slug);

	for (const key of FORWARDED_FILTERS) {
		const value = url.searchParams.get(key)?.trim();
		if (value) {
			search.set(key, value);
		}
	}

	throw redirect(307, `/events?${search.toString()}`);
}) satisfies PageServerLoad;
