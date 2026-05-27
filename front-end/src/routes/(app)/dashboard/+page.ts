import { listTasks, getTaskCounts } from '$lib/api/tasks';
import { listMachines } from '$lib/api/machines';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, depends }) => {
	depends('malbox:tasks');
	depends('malbox:machines');
	const [tasks, counts, machines] = await Promise.all([
		listTasks(fetch, { limit: 10 }),
		getTaskCounts(fetch),
		listMachines(fetch)
	]);
	return { tasks, counts, machines };
};
