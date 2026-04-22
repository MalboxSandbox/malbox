import { listMachines } from '$lib/api/machines';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, depends }) => {
	depends('malbox:machines');
	const machines = await listMachines(fetch);
	return { machines };
};
