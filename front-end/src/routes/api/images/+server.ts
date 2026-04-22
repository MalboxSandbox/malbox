import { proxy, proxyJson } from '$lib/server/proxy';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async () => proxy('/v1/images');

export const POST: RequestHandler = async ({ request }) => {
	const body = await request.json();
	return proxyJson('/v1/images', 'POST', body);
};
