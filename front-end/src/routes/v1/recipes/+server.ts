import { proxy } from '$lib/server/proxy';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ url }) => {
	return proxy(`/v1/recipes${url.search}`);
};

export const POST: RequestHandler = async ({ request }) => {
	return proxy('/v1/recipes', {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: request.body,
		// @ts-expect-error duplex required for streaming
		duplex: 'half'
	});
};
