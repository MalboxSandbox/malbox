import { requestJson, type FetchLike } from './client';
import type {
	Task,
	PaginatedTasks,
	TaskCounts,
	TaskReport,
	TaskTree,
	Sample,
	SinglePluginReport,
	SampleLookup,
	CreateTaskFromFileRequest,
	CreateTaskFromUrlRequest,
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

// Full report with sections stripped from plugin envelopes. Without
// `artifactId` the plugin views are the root artifact's and the aggregate
// covers the whole artifact tree.
export async function getTaskReport(
	fetchFn: FetchLike,
	id: number,
	artifactId?: number
): Promise<TaskReport> {
	const qs = buildQueryString({ artifact_id: artifactId });
	return requestJson<TaskReport>(fetchFn, `/v1/tasks/${id}/report${qs}`);
}

// Full report for a single plugin (with sections)
export async function getPluginReport(
	fetchFn: FetchLike,
	taskId: number,
	pluginName: string,
	artifactId?: number
): Promise<SinglePluginReport> {
	const qs = buildQueryString({ artifact_id: artifactId });
	return requestJson<SinglePluginReport>(
		fetchFn,
		`/v1/tasks/${taskId}/report/plugins/${encodeURIComponent(pluginName)}${qs}`
	);
}

export async function getTaskTree(fetchFn: FetchLike, id: number): Promise<TaskTree> {
	return requestJson<TaskTree>(fetchFn, `/v1/tasks/${id}/tree`);
}

export async function cancelArtifact(
	fetchFn: FetchLike,
	taskId: number,
	artifactId: number,
	descendants: 'cancel' | 'keep' = 'cancel'
): Promise<{ status: string }> {
	return requestJson(fetchFn, `/v1/tasks/${taskId}/artifacts/${artifactId}/cancel`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ descendants })
	});
}

export async function cancelJob(
	fetchFn: FetchLike,
	taskId: number,
	jobId: number
): Promise<{ status: string }> {
	return requestJson(fetchFn, `/v1/tasks/${taskId}/jobs/${jobId}/cancel`, { method: 'POST' });
}

export async function rescanArtifact(
	fetchFn: FetchLike,
	taskId: number,
	artifactId: number,
	opts: { plugins?: string[]; ignore_cache?: boolean } = {}
): Promise<{ status: string }> {
	return requestJson(fetchFn, `/v1/tasks/${taskId}/artifacts/${artifactId}/rescan`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify(opts)
	});
}

export async function promoteArtifact(
	fetchFn: FetchLike,
	taskId: number,
	artifactId: number
): Promise<{ task_id: number; root_artifact_id: number; sample: Sample & { id: number } }> {
	return requestJson(fetchFn, `/v1/tasks/${taskId}/artifacts/${artifactId}/promote`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({})
	});
}

export async function setArtifactPriority(
	fetchFn: FetchLike,
	taskId: number,
	artifactId: number,
	priority: number,
	subtree = false
): Promise<unknown> {
	return requestJson(fetchFn, `/v1/tasks/${taskId}/artifacts/${artifactId}`, {
		method: 'PATCH',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ priority, subtree })
	});
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
	if (req.timeout !== undefined) form.append('timeout', String(req.timeout));
	if (req.priority !== undefined) form.append('priority', String(req.priority));
	if (req.platform !== undefined) form.append('platform', req.platform);
	if (req.tags !== undefined) form.append('tags', req.tags);
	if (req.owner !== undefined) form.append('owner', req.owner);
	if (req.target_filename !== undefined) form.append('target_filename', req.target_filename);
	if (req.plugins !== undefined) form.append('plugins', req.plugins);
	if (req.snapshot_id !== undefined) form.append('snapshot_id', req.snapshot_id);
	if (req.machine_id !== undefined) form.append('machine_id', String(req.machine_id));
	if (req.params !== undefined) form.append('params', JSON.stringify(req.params));

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
