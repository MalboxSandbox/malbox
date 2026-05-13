import { proxy } from '$lib/server/proxy';
import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ params }) => {
	return proxy(`/v1/tasks/${encodeURIComponent(params.id)}/cancel`, { method: 'POST' });
};
