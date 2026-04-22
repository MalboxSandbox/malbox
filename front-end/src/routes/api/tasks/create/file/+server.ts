import { proxy } from '$lib/server/proxy';
import type { RequestHandler } from './$types';

// Large uploads rely on BODY_SIZE_LIMIT being set in the environment
// (see .env.example). This handler streams the incoming body to the back-end
// without buffering the whole file in memory.

export const POST: RequestHandler = async ({ request }) => {
	const contentType = request.headers.get('content-type') ?? '';
	return proxy('/v1/tasks/create/file', {
		method: 'POST',
		headers: { 'content-type': contentType },
		body: request.body,
		// @ts-expect-error duplex is required by undici for streaming bodies
		duplex: 'half'
	});
};
