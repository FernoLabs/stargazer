import { redirect } from '@sveltejs/kit';
import type { RequestHandler } from './$types';

export const GET = (({ params }) => {
	throw redirect(308, `/protocols/${params.slug}`);
}) satisfies RequestHandler;
