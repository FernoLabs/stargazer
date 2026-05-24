import { error } from '@sveltejs/kit';
import {
	eventAlertLabel,
	eventPreviewDescription,
	eventPreviewTitle
} from '$lib/event-preview';
import { backendRequest, publicBackendUrl, readBackendError } from '$lib/server/backend';
import type { ChainEvent } from '$lib/types';
import type { PageServerLoad } from './$types';

export const load = (async ({ fetch, params, parent, url }) => {
	const app = await parent();
	const id = params.id.trim();
	if (!id) throw error(404, 'Event not found');

	const response = await backendRequest(fetch, `/api/events/${encodeURIComponent(id)}`);
	if (!response.ok) {
		if (response.status === 404) throw error(404, 'Event not found');
		throw error(response.status, await readBackendError(response));
	}

	const event = (await response.json()) as ChainEvent;
	const title = eventPreviewTitle(event, app.defiProtocols);

	return {
		event,
		preview: {
			title,
			pageTitle: `${title} | FireWatch`,
			description: eventPreviewDescription(event),
			alertLabel: eventAlertLabel(event),
			url: url.href,
			imageUrl: new URL(
				publicBackendUrl(`/api/events/${encodeURIComponent(id)}/preview-card.png`),
				url.origin
			).href
		}
	};
}) satisfies PageServerLoad;
