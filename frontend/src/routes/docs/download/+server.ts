import { redirect } from '@sveltejs/kit';

export function GET(): never {
	redirect(307, '/docs/stargazer-technical-documentation.pdf');
}
