import { error } from '@sveltejs/kit';
import {
	eventAlertLabel,
	eventAlertLevel,
	eventPreviewDescription,
	eventPreviewTitle
} from '$lib/event-preview';
import { backendRequest, readBackendError } from '$lib/server/backend';
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
	const alertLevel = eventAlertLevel(event);

	return {
		event,
		preview: {
			title,
			pageTitle: `${title} | FireWatch`,
			description: eventPreviewDescription(event),
			alertLabel: eventAlertLabel(event),
			url: url.href,
			imageUrl: `${url.origin}/brand/firewatch-preview-${alertLevel}.png`
		}
	};
}) satisfies PageServerLoad;
