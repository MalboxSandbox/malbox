import { proxy } from '$lib/server/proxy';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ url }) => {
	return proxy(`/v1/tasks/count${url.search}`);
};
