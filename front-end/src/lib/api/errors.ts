export type ApiErrorKind =
	| 'validation'
	| 'not_found'
	| 'conflict'
	| 'server'
	| 'network'
	| 'unknown';

export class ApiError extends Error {
	readonly status: number;
	readonly kind: ApiErrorKind;
	readonly fieldErrors?: Record<string, string[]>;
	readonly raw: unknown;

	constructor(
		status: number,
		kind: ApiErrorKind,
		message: string,
		raw: unknown,
		fieldErrors?: Record<string, string[]>
	) {
		super(message);
		this.name = 'ApiError';
		this.status = status;
		this.kind = kind;
		this.fieldErrors = fieldErrors;
		this.raw = raw;
	}
}

export function kindFromStatus(status: number): ApiErrorKind {
	if (status === 422) return 'validation';
	if (status === 404) return 'not_found';
	if (status === 409) return 'conflict';
	if (status === 502) return 'network';
	if (status >= 500) return 'server';
	return 'unknown';
}

export function isApiError(err: unknown): err is ApiError {
	return err instanceof ApiError;
}
