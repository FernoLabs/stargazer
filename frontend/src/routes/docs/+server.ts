import { redirect } from '@sveltejs/kit';

// /docs is the documentation entry point; the technical architecture is its first page.
export function GET(): never {
	redirect(307, '/docs/architecture');
}
