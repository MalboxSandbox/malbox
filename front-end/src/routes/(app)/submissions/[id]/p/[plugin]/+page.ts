import { error } from '@sveltejs/kit';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params, parent }) => {
	const { report } = await parent();
	const name = decodeURIComponent(params.plugin);
	const view = report.plugins.find((p) => p.plugin_name === name);
	if (!view) throw error(404, `Plugin "${name}" not found for this task`);
	return { view, sample: report.task.sample ?? null };
};
