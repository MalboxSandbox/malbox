import { proxy } from '$lib/server/proxy';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ params }) =>
	proxy(`/v1/machines/${encodeURIComponent(params.id)}`);
