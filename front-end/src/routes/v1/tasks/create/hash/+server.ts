import { proxy } from '$lib/server/proxy';
import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ request }) => {
	const body = await request.text();
	return proxy('/v1/tasks/create/hash', {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body
	});
};
