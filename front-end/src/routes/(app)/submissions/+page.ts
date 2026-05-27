import { listTasks } from '$lib/api/tasks';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, depends, url }) => {
	depends('malbox:tasks');

	const filter = url.searchParams.get('filter') ?? 'all';
	const statusParam = filterToStatus(filter);

	const tasks = await listTasks(fetch, {
		status: statusParam,
		limit: 50
	});

	return { tasks, filter };
};

function filterToStatus(filter: string): string | undefined {
	switch (filter) {
		case 'finished':
			return 'completed';
		case 'pending':
			return 'pending,running,initializing,preparing_resources,stopping';
		default:
			return undefined;
	}
}
