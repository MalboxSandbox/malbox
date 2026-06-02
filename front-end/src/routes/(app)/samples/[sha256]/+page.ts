import { error } from '@sveltejs/kit';
import { getSampleReport } from '$lib/api/samples';
import { getTaskReport, getPluginReport } from '$lib/api/tasks';
import { isApiError } from '$lib/api/errors';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, params, parent, depends }) => {
	depends('malbox:sample-report');
	const sha256 = params.sha256;
	const { overview } = await parent();

	let sampleReport;
	try {
		sampleReport = await getSampleReport(fetch, sha256);
	} catch (err) {
		if (isApiError(err) && err.status === 404) {
			throw error(404, 'Sample report not found');
		}
		throw err;
	}

	const completedTasks = overview.tasks.filter(
		(t) => t.status === 'completed' || t.status === 'running'
	);
	const taskReports = (
		await Promise.all(completedTasks.map((t) => getTaskReport(fetch, t.id).catch(() => null)))
	).filter((r): r is NonNullable<typeof r> => r !== null);

	// getTaskReport strips sections from plugin envelopes for payload size.
	// Backfill full plugin reports (with sections) in parallel.
	const enrichedTaskReports = await Promise.all(
		taskReports.map(async (tr) => {
			const enrichedPlugins = await Promise.all(
				tr.plugins.map(async (p) => {
					if (p.failed) return p;
					try {
						const full = await getPluginReport(fetch, tr.task.id, p.plugin_name);
						return { ...p, report: full.report, artifacts: full.artifacts };
					} catch {
						return p;
					}
				})
			);
			return { ...tr, plugins: enrichedPlugins };
		})
	);

	return {
		sampleReport,
		taskReports: enrichedTaskReports,
		tasks: overview.tasks,
		sample: overview.sample,
		aggregate: overview.aggregate
	};
};
