import { proxy } from '$lib/server/proxy';
import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ params, request }) => {
	return proxy(`/v1/tasks/create/sample/${encodeURIComponent(params.id)}`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: await request.text()
	});
};
