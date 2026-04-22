import { listPlugins } from '$lib/api/plugins';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, depends }) => {
	depends('malbox:plugins');
	const plugins = await listPlugins(fetch);
	return { plugins };
};
