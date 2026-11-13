import { redirect } from '@sveltejs/kit';

export function GET(): never {
	// The August 2026 PDF is retired; the current architecture lives on one page.
	redirect(308, '/docs/architecture');
}
