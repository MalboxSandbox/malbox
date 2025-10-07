export interface Submission {
	id: string;
	date: string;
	time: string;
	type: 'url' | 'file' | 'hash';
	value: string; // URL, filename, or hash value
	status: 'finished' | 'pending';
	progress: number;
	total: number;
	apiKey: string;
}

export interface ActivityData {
	day: string;
	value: number;
}

export interface Statistics {
	totalSubmissions: number;
	clean: number; // Threat score 0-3
	suspicious: number; // Threat score 4-6
	malicious: number; // Threat score 7-10
}

export type TimeRange = 'day' | 'week' | 'month';
