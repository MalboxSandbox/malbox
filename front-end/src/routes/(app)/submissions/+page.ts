import { listTasks } from '$lib/api/tasks';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, depends }) => {
	depends('malbox:tasks');
	const tasks = await listTasks(fetch);
	return { tasks };
};
