export interface SubmissionItem {
	id: string;
	date: string;
	time: string;
	type: 'url' | 'file' | 'hash';
	value: string;
	status: 'finished' | 'pending';
	progress: number;
	total: number;
	apiKey: string | null;
}

export type SubmissionFilter = 'all' | 'finished' | 'pending';
