import { proxyJson } from '$lib/server/proxy';
import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ params, request }) => {
	const body = await request.json();
	return proxyJson(`/v1/machines/${encodeURIComponent(params.id)}/provision`, 'POST', body);
};
