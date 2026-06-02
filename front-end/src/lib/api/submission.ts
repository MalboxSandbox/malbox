import { isApiError } from './errors';
import type { FetchLike } from './client';
import { lookupSample } from './tasks';

export type HashKind = 'md5' | 'sha1' | 'sha256' | 'sha512';

const HASH_PATTERN = /^[a-fA-F0-9]{32}$|^[a-fA-F0-9]{40}$|^[a-fA-F0-9]{64}$|^[a-fA-F0-9]{128}$/;

/** True when the trimmed input is a hex string of MD5/SHA-1/SHA-256/SHA-512 length. */
export function looksLikeHash(value: string): boolean {
	return HASH_PATTERN.test(value.trim());
}

/** Maps a hash to its algorithm by length. Assumes looksLikeHash(value) is true. */
export function hashType(value: string): HashKind {
	const len = value.trim().length;
	if (len === 32) return 'md5';
	if (len === 40) return 'sha1';
	if (len === 128) return 'sha512';
	return 'sha256';
}

export type HashResolution =
	| { status: 'found'; href: string }
	| { status: 'not_found' }
	| { status: 'error'; message: string };

/**
 * Looks a hash up against the sample database and decides where to send the user.
 * On a hit, returns the sample page href so the caller can navigate there directly
 * instead of rendering submission UI inline. The lookup is injected for testing.
 */
export async function resolveHashToSample(
	fetchFn: FetchLike,
	value: string,
	lookup: typeof lookupSample = lookupSample
): Promise<HashResolution> {
	const trimmed = value.trim();
	try {
		const { sample } = await lookup(fetchFn, hashType(trimmed), trimmed);
		return { status: 'found', href: `/samples/${sample.sha256}` };
	} catch (err) {
		if (isApiError(err) && err.status === 404) {
			return { status: 'not_found' };
		}
		return {
			status: 'error',
			message: isApiError(err) ? err.message : 'Lookup failed.'
		};
	}
}
