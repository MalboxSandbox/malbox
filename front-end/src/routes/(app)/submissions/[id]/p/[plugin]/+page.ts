import { error } from '@sveltejs/kit';
import { getPluginReport } from '$lib/api/tasks';
import { isApiError } from '$lib/api/errors';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params, parent, fetch }) => {
	const { task } = await parent();
	const name = decodeURIComponent(params.plugin);

	let view;
	try {
		view = await getPluginReport(fetch, task.id, name);
	} catch (err) {
		if (isApiError(err) && err.status === 404) {
			throw error(404, `Plugin "${name}" not found for this task`);
		}
		throw err;
	}

	return { view, sample: task.sample ?? null };
};
