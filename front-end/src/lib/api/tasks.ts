import { request, requestJson, type FetchLike } from './client';
import type {
	Task,
	TaskResult,
	TaskReport,
	CreateTaskFromFileRequest,
	CreateTaskFromUrlRequest,
	CreateTaskFromHashRequest
} from './types';

export async function listTasks(fetchFn: FetchLike): Promise<Task[]> {
	return requestJson<Task[]>(fetchFn, '/v1/tasks');
}

export async function getTask(fetchFn: FetchLike, id: number): Promise<Task> {
	return requestJson<Task>(fetchFn, `/v1/tasks/${id}`);
}

export async function getTaskResults(fetchFn: FetchLike, id: number): Promise<TaskResult[]> {
	return requestJson<TaskResult[]>(fetchFn, `/v1/tasks/${id}/results`);
}

export async function getTaskReport(fetchFn: FetchLike, id: number): Promise<TaskReport> {
	return requestJson<TaskReport>(fetchFn, `/v1/tasks/${id}/report`);
}

/** Returns the raw Response so callers can choose JSON, blob, or download URL. */
export async function getResultContent(
	fetchFn: FetchLike,
	taskId: number,
	resultId: number
): Promise<Response> {
	return request(fetchFn, `/v1/tasks/${taskId}/results/${resultId}`);
}

export async function cancelTask(
	fetchFn: FetchLike,
	id: number
): Promise<{ status: string; task_id: number }> {
	return requestJson(fetchFn, `/v1/tasks/${id}/cancel`, { method: 'POST' });
}

export async function createTaskFromFile(
	fetchFn: FetchLike,
	req: CreateTaskFromFileRequest
): Promise<{ task_id: number }> {
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

	return requestJson<{ task_id: number }>(fetchFn, '/v1/tasks/create/file', {
		method: 'POST',
		body: form
	});
}

export async function createTaskFromUrl(
	fetchFn: FetchLike,
	req: CreateTaskFromUrlRequest
): Promise<{ task_id: number }> {
	return requestJson<{ task_id: number }>(fetchFn, '/v1/tasks/create/url', {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify(req)
	});
}

export async function createTaskFromHash(
	fetchFn: FetchLike,
	req: CreateTaskFromHashRequest
): Promise<{ task_id: number }> {
	return requestJson<{ task_id: number }>(fetchFn, '/v1/tasks/create/hash', {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify(req)
	});
}
