import { listTasks } from '$lib/api/tasks';
import { listMachines } from '$lib/api/machines';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, depends }) => {
	depends('malbox:tasks');
	depends('malbox:machines');
	const [tasks, machines] = await Promise.all([listTasks(fetch), listMachines(fetch)]);
	return { tasks, machines };
};
