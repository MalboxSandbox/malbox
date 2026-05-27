import { error } from '@sveltejs/kit';
import { getTask, getTaskReport } from '$lib/api/tasks';
import { isApiError } from '$lib/api/errors';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ fetch, params, depends }) => {
	const id = Number(params.id);
	if (!Number.isFinite(id)) throw error(400, 'Invalid task id');
	depends('malbox:report');

	let task;
	try {
		task = await getTask(fetch, id);
	} catch (err) {
		if (isApiError(err) && err.status === 404) throw error(404, `Task ${id} not found`);
		throw err;
	}

	const report = getTaskReport(fetch, id).catch((err) => {
		if (isApiError(err) && err.status === 404) throw error(404, `Task ${id} not found`);
		throw err;
	});

	return { task, report };
};
