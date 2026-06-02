import { error } from '@sveltejs/kit';
import { getTask, getTaskReport } from '$lib/api/tasks';
import { isApiError } from '$lib/api/errors';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, params, depends }) => {
	const taskId = Number(params.taskId);
	if (!Number.isFinite(taskId)) throw error(400, 'Invalid task id');
	depends('malbox:run-report');

	let task;
	try {
		task = await getTask(fetch, taskId);
	} catch (err) {
		if (isApiError(err) && err.status === 404) throw error(404, `Task ${taskId} not found`);
		throw err;
	}

	const report = getTaskReport(fetch, taskId).catch((err) => {
		if (isApiError(err) && err.status === 404) throw error(404, `Task ${taskId} not found`);
		throw err;
	});

	return { task, report, taskId };
};
