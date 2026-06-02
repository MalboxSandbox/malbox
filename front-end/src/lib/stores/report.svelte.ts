import { SvelteMap } from 'svelte/reactivity';
import type { TaskReport, TaskSummary, SampleInfo, PluginReportView } from '$lib/api/types';

let current = $state<TaskReport | null>(null);

let sampleState = $state<{
	sha256: string;
	sample: SampleInfo;
	tasks: TaskSummary[];
	taskReports: TaskReport[];
	activeRunId: string;
} | null>(null);

const activePlugins = $derived.by((): PluginReportView[] => {
	if (!sampleState) return [];
	const { taskReports, activeRunId } = sampleState;

	if (activeRunId === 'combined') {
		const map = new SvelteMap<string, PluginReportView>();
		for (const report of taskReports) {
			for (const p of report.plugins) {
				if (!p.failed) map.set(p.plugin_name, p);
			}
		}
		return [...map.values()];
	}

	const report = taskReports.find((r) => String(r.task.id) === activeRunId);
	return report?.plugins.filter((p) => !p.failed) ?? [];
});

export const reportStore = {
	get current() {
		return current;
	},
	set(report: TaskReport | null) {
		current = report;
	},

	get samplePage() {
		return sampleState;
	},
	get activePlugins() {
		return activePlugins;
	},
	setSamplePage(state: typeof sampleState) {
		sampleState = state;
	},
	setActiveRun(id: string) {
		if (sampleState) sampleState = { ...sampleState, activeRunId: id };
	}
};
