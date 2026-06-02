import { error } from '@sveltejs/kit';
import { getPluginReport, getTask } from '$lib/api/tasks';
import { isApiError } from '$lib/api/errors';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, params }) => {
	const taskId = Number(params.taskId);
	if (!Number.isFinite(taskId)) throw error(400, 'Invalid task id');

	let task;
	try {
		task = await getTask(fetch, taskId);
	} catch (err) {
		if (isApiError(err) && err.status === 404) throw error(404, `Task ${taskId} not found`);
		throw err;
	}

	let view;
	try {
		view = await getPluginReport(fetch, taskId, params.plugin);
	} catch (err) {
		if (isApiError(err) && err.status === 404) {
			throw error(404, `No report for plugin '${params.plugin}'`);
		}
		throw err;
	}

	return { task, view, sample: task.sample };
};
