import { request, requestJson, type FetchLike } from './client';
import type {
	Task,
	PaginatedTasks,
	TaskCounts,
	TaskResult,
	TaskReport,
	SinglePluginReport,
	SampleLookup,
	CreateTaskFromFileRequest,
	CreateTaskFromUrlRequest,
	CreateTaskFromHashRequest,
	RescanSampleRequest
} from './types';

export interface ListTasksParams {
	status?: string;
	platform?: string;
	owner?: string;
	tag?: string;
	after?: string;
	before?: string;
	cursor?: string;
	limit?: number;
}

function buildQueryString(params: Record<string, string | number | undefined>): string {
	const entries = Object.entries(params).filter(
		(entry): entry is [string, string | number] => entry[1] !== undefined
	);
	if (entries.length === 0) return '';
	return '?' + entries.map(([k, v]) => `${k}=${encodeURIComponent(v)}`).join('&');
}

export async function listTasks(
	fetchFn: FetchLike,
	params: ListTasksParams = {}
): Promise<PaginatedTasks> {
	const qs = buildQueryString(params as Record<string, string | number | undefined>);
	return requestJson<PaginatedTasks>(fetchFn, `/v1/tasks${qs}`);
}

export async function getTaskCounts(
	fetchFn: FetchLike,
	params: Omit<ListTasksParams, 'cursor' | 'limit'> = {}
): Promise<TaskCounts> {
	const qs = buildQueryString(params as Record<string, string | number | undefined>);
	return requestJson<TaskCounts>(fetchFn, `/v1/tasks/count${qs}`);
}

export async function getTask(fetchFn: FetchLike, id: number): Promise<Task> {
	return requestJson<Task>(fetchFn, `/v1/tasks/${id}`);
}

export async function getTaskResults(fetchFn: FetchLike, id: number): Promise<TaskResult[]> {
	return requestJson<TaskResult[]>(fetchFn, `/v1/tasks/${id}/results`);
}

// Full report with sections stripped from plugin envelopes
export async function getTaskReport(fetchFn: FetchLike, id: number): Promise<TaskReport> {
	return requestJson<TaskReport>(fetchFn, `/v1/tasks/${id}/report`);
}

// Full report for a single plugin (with sections)
export async function getPluginReport(
	fetchFn: FetchLike,
	taskId: number,
	pluginName: string
): Promise<SinglePluginReport> {
	return requestJson<SinglePluginReport>(
		fetchFn,
		`/v1/tasks/${taskId}/report/plugins/${encodeURIComponent(pluginName)}`
	);
}

/** Returns the raw Response so callers can choose JSON, blob, or download URL. */
export async function getResultContent(
	fetchFn: FetchLike,
	taskId: number,
	resultId: number
): Promise<Response> {
	return request(fetchFn, `/v1/tasks/${taskId}/results/${resultId}/content`);
}

export async function cancelTask(
	fetchFn: FetchLike,
	id: number
): Promise<{ status: string; task_id: number }> {
	return requestJson(fetchFn, `/v1/tasks/${id}/cancel`, { method: 'POST' });
}

export async function lookupSample(
	fetchFn: FetchLike,
	hashType: 'md5' | 'sha1' | 'sha256' | 'sha512',
	hashValue: string
): Promise<SampleLookup> {
	return requestJson<SampleLookup>(
		fetchFn,
		`/v1/samples?${hashType}=${encodeURIComponent(hashValue)}`
	);
}

export async function createTaskFromFile(
	fetchFn: FetchLike,
	req: CreateTaskFromFileRequest
): Promise<{ task_id: number; sha256: string }> {
	const form = new FormData();
	form.append('file', req.file, req.file.name);
	if (req.package !== undefined) form.append('package', req.package);
	if (req.timeout !== undefined) form.append('timeout', String(req.timeout));
	if (req.priority !== undefined) form.append('priority', String(req.priority));
	if (req.platform !== undefined) form.append('platform', req.platform);
	if (req.tags !== undefined) form.append('tags', req.tags);
	if (req.owner !== undefined) form.append('owner', req.owner);
	if (req.enforce_timeout !== undefined)
		form.append('enforce_timeout', String(req.enforce_timeout));
	if (req.target_filename !== undefined) form.append('target_filename', req.target_filename);
	if (req.plugins !== undefined) form.append('plugins', req.plugins);
	if (req.snapshot_id !== undefined) form.append('snapshot_id', req.snapshot_id);

	const res = await requestJson<{ task_id: number; sample: { sha256: string } }>(
		fetchFn,
		'/v1/tasks',
		{ method: 'POST', body: form }
	);
	return { task_id: res.task_id, sha256: res.sample.sha256 };
}

export async function createTaskFromUrl(
	fetchFn: FetchLike,
	req: CreateTaskFromUrlRequest
): Promise<{ task_id: number }> {
	const { url, ...rest } = req;
	return requestJson<{ task_id: number }>(fetchFn, '/v1/tasks', {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ source: { url }, ...rest })
	});
}

export async function createTaskFromHash(
	fetchFn: FetchLike,
	req: CreateTaskFromHashRequest
): Promise<{ task_id: number }> {
	const { hash, ...rest } = req;
	return requestJson<{ task_id: number }>(fetchFn, '/v1/tasks', {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ source: { hash }, ...rest })
	});
}

export async function rescanSample(
	fetchFn: FetchLike,
	sampleId: number,
	req: RescanSampleRequest = {}
): Promise<{ task_id: number; sha256: string }> {
	const res = await requestJson<{ task_id: number; sample: { sha256: string } }>(
		fetchFn,
		'/v1/tasks',
		{
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify({ source: { sample_id: sampleId }, ...req })
		}
	);
	return { task_id: res.task_id, sha256: res.sample.sha256 };
}
