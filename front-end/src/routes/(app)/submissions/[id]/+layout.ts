import { error, redirect } from '@sveltejs/kit';
import { getTask } from '$lib/api/tasks';
import { isApiError } from '$lib/api/errors';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ fetch, params }) => {
	const id = Number(params.id);
	if (!Number.isFinite(id)) throw error(400, 'Invalid task id');

	let task;
	try {
		task = await getTask(fetch, id);
	} catch (err) {
		if (isApiError(err) && err.status === 404) throw error(404, `Task ${id} not found`);
		throw err;
	}

	if (task.sample?.sha256) {
		redirect(301, `/samples/${task.sample.sha256}?run=${id}`);
	}

	throw error(404, 'Sample not found for this task');
};
