import { proxy } from '$lib/server/proxy';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ url }) => {
	const qs = url.searchParams.toString();
	return proxy(`/v1/snapshots?${qs}`);
};
