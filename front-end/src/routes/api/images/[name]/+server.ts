import { proxy } from '$lib/server/proxy';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ params }) =>
	proxy(`/v1/images/${encodeURIComponent(params.name)}`);

export const DELETE: RequestHandler = async ({ params }) =>
	proxy(`/v1/images/${encodeURIComponent(params.name)}`, { method: 'DELETE' });
