import { describe, it, expect, vi } from 'vitest';
import { looksLikeHash, hashType, resolveHashToSample } from './submission';
import { ApiError } from './errors';
import type { FetchLike } from './client';
import type { lookupSample } from './tasks';

// The lookup function is injected, so these tests never hit the network.
const noopFetch = (() => {
	throw new Error('fetch must not be called directly here');
}) as unknown as FetchLike;

function fakeLookup(impl: unknown): typeof lookupSample {
	return impl as unknown as typeof lookupSample;
}

describe('looksLikeHash', () => {
	it('accepts md5/sha1/sha256/sha512-length hex (with surrounding whitespace)', () => {
		expect(looksLikeHash('d41d8cd98f00b204e9800998ecf8427e')).toBe(true);
		expect(looksLikeHash('a'.repeat(40))).toBe(true);
		expect(looksLikeHash('a'.repeat(64))).toBe(true);
		expect(looksLikeHash('a'.repeat(128))).toBe(true);
		expect(looksLikeHash(`  ${'a'.repeat(64)}  `)).toBe(true);
	});

	it('rejects URLs, non-hex, and wrong-length strings', () => {
		expect(looksLikeHash('https://example.com')).toBe(false);
		expect(looksLikeHash('not-a-hash')).toBe(false);
		expect(looksLikeHash('a'.repeat(33))).toBe(false);
		expect(looksLikeHash('z'.repeat(64))).toBe(false);
	});
});

describe('hashType', () => {
	it('classifies by length', () => {
		expect(hashType('a'.repeat(32))).toBe('md5');
		expect(hashType('a'.repeat(40))).toBe('sha1');
		expect(hashType('a'.repeat(64))).toBe('sha256');
		expect(hashType('a'.repeat(128))).toBe('sha512');
	});
});

describe('resolveHashToSample', () => {
	const sha256 = 'a'.repeat(64);

	it('resolves to the sample page href when the lookup succeeds', async () => {
		const lookup = vi.fn().mockResolvedValue({ sample: { sha256: 'deadbeef' }, task_ids: [1, 2] });
		const result = await resolveHashToSample(noopFetch, sha256, fakeLookup(lookup));
		expect(result).toEqual({ status: 'found', href: '/samples/deadbeef' });
		expect(lookup).toHaveBeenCalledWith(noopFetch, 'sha256', sha256);
	});

	it('reports not_found on a 404 (there is no sample to redirect to)', async () => {
		const lookup = vi.fn().mockRejectedValue(new ApiError(404, 'not_found', 'nope', null));
		const result = await resolveHashToSample(noopFetch, sha256, fakeLookup(lookup));
		expect(result).toEqual({ status: 'not_found' });
	});

	it('surfaces a message on non-404 failures', async () => {
		const lookup = vi.fn().mockRejectedValue(new ApiError(500, 'server', 'boom', null));
		const result = await resolveHashToSample(noopFetch, sha256, fakeLookup(lookup));
		expect(result).toEqual({ status: 'error', message: 'boom' });
	});
});
