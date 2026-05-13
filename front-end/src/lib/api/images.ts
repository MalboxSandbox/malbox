import { request, requestJson, type FetchLike } from './client';
import type { Image, RegisterImageRequest } from './types';

export async function listImages(fetchFn: FetchLike): Promise<Image[]> {
	return requestJson<Image[]>(fetchFn, '/v1/images');
}

export async function getImage(fetchFn: FetchLike, name: string): Promise<Image> {
	return requestJson<Image>(fetchFn, `/v1/images/${encodeURIComponent(name)}`);
}

export async function registerImage(
	fetchFn: FetchLike,
	req: RegisterImageRequest
): Promise<{ id: string; name: string; path: string; available: boolean }> {
	return requestJson(fetchFn, '/v1/images', {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify(req)
	});
}

export async function deleteImage(fetchFn: FetchLike, name: string): Promise<void> {
	await request(fetchFn, `/v1/images/${encodeURIComponent(name)}`, { method: 'DELETE' });
}
