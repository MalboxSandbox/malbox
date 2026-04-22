import { ApiError, kindFromStatus } from './errors';

export type FetchLike = typeof fetch;

export interface RequestOptions {
	method?: string;
	body?: BodyInit;
	headers?: Record<string, string>;
	signal?: AbortSignal;
}

/**
 * Low-level request helper. Returns the Response on success (2xx),
 * throws ApiError on non-2xx or network failure.
 *
 * Callers decide how to parse the body (JSON, blob, text).
 */
export async function request(
	fetchFn: FetchLike,
	path: string,
	opts: RequestOptions = {}
): Promise<Response> {
	let res: Response;
	try {
		res = await fetchFn(path, {
			method: opts.method ?? 'GET',
			body: opts.body,
			headers: opts.headers,
			signal: opts.signal
		});
	} catch (e) {
		throw new ApiError(0, 'network', e instanceof Error ? e.message : 'network failure', e);
	}

	if (res.ok) return res;

	const kind = kindFromStatus(res.status);
	let raw: unknown = null;
	let message = res.statusText || `HTTP ${res.status}`;
	let fieldErrors: Record<string, string[]> | undefined;

	const contentType = res.headers.get('content-type') ?? '';
	if (contentType.includes('application/json')) {
		try {
			raw = await res.json();
		} catch {
			/* keep raw null */
		}
	} else {
		try {
			const text = await res.text();
			raw = text;
			if (text) message = text;
		} catch {
			/* ignore */
		}
	}

	if (raw && typeof raw === 'object') {
		const obj = raw as Record<string, unknown>;
		if (typeof obj.error === 'string') message = obj.error;
		if (obj.errors && typeof obj.errors === 'object') {
			fieldErrors = obj.errors as Record<string, string[]>;
		}
	}

	throw new ApiError(res.status, kind, message, raw, fieldErrors);
}

export async function requestJson<T>(
	fetchFn: FetchLike,
	path: string,
	opts: RequestOptions = {}
): Promise<T> {
	const res = await request(fetchFn, path, opts);
	if (res.status === 204) return undefined as T;
	return (await res.json()) as T;
}
