import type { PageServerLoad } from './$types';

export const load = (({ setHeaders }) => {
	setHeaders({
		'cache-control': 'private, no-store',
		'x-robots-tag': 'noindex, nofollow, noarchive'
	});

	return {};
}) satisfies PageServerLoad;
