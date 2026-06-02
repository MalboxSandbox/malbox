import { proxy } from '$lib/server/proxy';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ params }) => {
	return proxy(`/v1/samples/${encodeURIComponent(params.sha256)}/report`);
};
