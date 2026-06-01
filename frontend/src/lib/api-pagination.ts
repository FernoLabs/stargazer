import type { ApiLinks } from './types';

export function nextPageOffset(links: ApiLinks | null | undefined) {
	return pageOffsetFromLink(links?.next);
}

function pageOffsetFromLink(link: string | null | undefined) {
	if (!link) return null;

	const url = new URL(link, 'https://firewatch.local');
	const rawOffset = url.searchParams.get('page[offset]');
	if (!rawOffset) return null;

	const offset = Number.parseInt(rawOffset, 10);
	return Number.isFinite(offset) ? offset : null;
}
