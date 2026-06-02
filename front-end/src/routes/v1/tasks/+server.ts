import { proxy } from '$lib/server/proxy';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ url }) => {
	return proxy(`/v1/tasks${url.search}`);
};

// Single creation endpoint. Streams the body (multipart upload or JSON
// reference) straight through to the back-end, preserving Content-Type so the
// back-end's content-negotiating extractor can dispatch.
export const POST: RequestHandler = async ({ request }) => {
	const contentType = request.headers.get('content-type') ?? '';
	return proxy('/v1/tasks', {
		method: 'POST',
		headers: { 'content-type': contentType },
		body: request.body,
		// @ts-expect-error duplex is required by undici for streaming bodies
		duplex: 'half'
	});
};
