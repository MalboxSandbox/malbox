import { getBackendUrl } from '$lib/config';
import { error } from '@sveltejs/kit';

/**
 * Forward a request to the back-end. Returns the back-end's Response unchanged
 * (status, headers, body). On network failure, throws a 502 error for SvelteKit.
 */
export async function proxy(path: string, init: RequestInit = {}): Promise<Response> {
	const url = `${getBackendUrl()}${path}`;
	try {
		return await fetch(url, init);
	} catch (e) {
		console.error(`[proxy] back-end unreachable: ${url}`, e);
		throw error(502, 'back-end unreachable');
	}
}

/** Forward with explicit JSON body. */
export async function proxyJson(
	path: string,
	method: string,
	body: unknown,
	headers: Record<string, string> = {}
): Promise<Response> {
	return proxy(path, {
		method,
		headers: { 'content-type': 'application/json', ...headers },
		body: JSON.stringify(body)
	});
}
