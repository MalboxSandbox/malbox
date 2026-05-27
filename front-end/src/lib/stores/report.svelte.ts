import type { TaskReport } from '$lib/api/types';

let current = $state<TaskReport | null>(null);

export const reportStore = {
	get current() {
		return current;
	},
	set(report: TaskReport | null) {
		current = report;
	}
};
