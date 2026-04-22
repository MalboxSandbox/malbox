import { error } from '@sveltejs/kit';
import { getTaskReport } from '$lib/api/tasks';
import { isApiError } from '$lib/api/errors';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ fetch, params, depends }) => {
	const id = Number(params.id);
	if (!Number.isFinite(id)) throw error(400, 'Invalid task id');
	depends('malbox:report');
	try {
		const report = await getTaskReport(fetch, id);
		return { report };
	} catch (err) {
		if (isApiError(err) && err.status === 404) throw error(404, `Task ${id} not found`);
		throw err;
	}
};
