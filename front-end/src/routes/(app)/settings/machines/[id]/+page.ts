import { error } from '@sveltejs/kit';
import { getMachine, listSnapshots, listProvisions } from '$lib/api/machines';
import { listPlugins } from '$lib/api/plugins';
import { isApiError } from '$lib/api/errors';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, params, depends }) => {
	const id = Number(params.id);
	if (!Number.isFinite(id)) throw error(400, 'Invalid machine id');
	depends('malbox:machines');
	try {
		const [machine, snapshots, provisions, guestPlugins] = await Promise.all([
			getMachine(fetch, id),
			listSnapshots(fetch, id),
			listProvisions(fetch, id),
			listPlugins(fetch, 'guest')
		]);
		return { machine, snapshots, provisions, guestPlugins };
	} catch (err) {
		if (isApiError(err) && err.status === 404) throw error(404, `Machine ${id} not found`);
		throw err;
	}
};
