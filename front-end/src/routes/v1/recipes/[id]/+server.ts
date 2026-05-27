import { proxy } from '$lib/server/proxy';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ params }) => {
	return proxy(`/v1/recipes/${encodeURIComponent(params.id)}`);
};

export const PUT: RequestHandler = async ({ params, request }) => {
	return proxy(`/v1/recipes/${encodeURIComponent(params.id)}`, {
		method: 'PUT',
		headers: { 'content-type': 'application/json' },
		body: request.body,
		// @ts-expect-error duplex required for streaming
		duplex: 'half'
	});
};

export const DELETE: RequestHandler = async ({ params }) => {
	return proxy(`/v1/recipes/${encodeURIComponent(params.id)}`, {
		method: 'DELETE'
	});
};
