import type { TaskStatus } from './types';

/** Split an ISO-ish timestamp into {date: 'YYYY-MM-DD', time: 'HH:MM'}. */
export function splitDateTime(isoish: string): { date: string; time: string } {
	const d = new Date(isoish);
	if (Number.isNaN(d.getTime())) {
		// Fallback: best-effort split of a `YYYY-MM-DD HH:MM:SS...` string.
		const m = isoish.match(/^(\d{4}-\d{2}-\d{2})[ T](\d{2}:\d{2})/);
		return m ? { date: m[1], time: m[2] } : { date: isoish, time: '' };
	}
	const pad = (n: number) => String(n).padStart(2, '0');
	return {
		date: `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`,
		time: `${pad(d.getHours())}:${pad(d.getMinutes())}`
	};
}

export function taskStatusLabel(status: TaskStatus): string {
	switch (status) {
		case 'pending':
			return 'Pending';
		case 'running':
			return 'Running';
		case 'completed':
			return 'Finished';
		case 'failed':
			return 'Failed';
		case 'cancelled':
			return 'Cancelled';
	}
}

export function isTerminalStatus(status: TaskStatus): boolean {
	return status === 'completed' || status === 'failed' || status === 'cancelled';
}

export function formatBytes(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
	if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
	return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}
