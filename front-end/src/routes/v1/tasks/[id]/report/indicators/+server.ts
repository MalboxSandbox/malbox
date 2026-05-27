import { proxy } from '$lib/server/proxy';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ params }) => {
	return proxy(`/v1/tasks/${encodeURIComponent(params.id)}/report/indicators`);
};
