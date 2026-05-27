import { requestJson, type FetchLike } from './client';
import type { CustomTransform, TransformKind } from './types';

export async function listCustomTransforms(fetchFn: FetchLike): Promise<CustomTransform[]> {
	return requestJson<CustomTransform[]>(fetchFn, '/v1/transforms');
}

export async function getCustomTransform(fetchFn: FetchLike, id: string): Promise<CustomTransform> {
	return requestJson<CustomTransform>(fetchFn, `/v1/transforms/${id}`);
}

export async function createCustomTransform(
	fetchFn: FetchLike,
	transform: {
		transform_id: string;
		name: string;
		category: string;
		kind: TransformKind;
		content: string;
	}
): Promise<CustomTransform> {
	return requestJson<CustomTransform>(fetchFn, '/v1/transforms', {
		method: 'POST',
		body: JSON.stringify(transform),
		headers: { 'Content-Type': 'application/json' }
	});
}

export async function updateCustomTransform(
	fetchFn: FetchLike,
	id: string,
	transform: { name: string; category: string; content: string; enabled?: boolean }
): Promise<CustomTransform> {
	return requestJson<CustomTransform>(fetchFn, `/v1/transforms/${id}`, {
		method: 'PUT',
		body: JSON.stringify(transform),
		headers: { 'Content-Type': 'application/json' }
	});
}

export async function deleteCustomTransform(fetchFn: FetchLike, id: string): Promise<void> {
	await requestJson(fetchFn, `/v1/transforms/${id}`, { method: 'DELETE' });
}
