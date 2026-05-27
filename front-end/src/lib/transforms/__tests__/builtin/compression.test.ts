import { describe, it, expect } from 'vitest';
import {
	gunzip,
	gzip,
	inflate,
	deflate,
	zlibCompress,
	zlibDecompress
} from '../../builtin/compression';

function textToBytes(s: string): Uint8Array {
	return new TextEncoder().encode(s);
}

function bytesToText(b: Uint8Array): string {
	return new TextDecoder().decode(b);
}

describe('gunzip / gzip', () => {
	it('decompresses gzip data', async () => {
		const compressed = await gzip.apply(textToBytes('Hello, compressed world!'), {});
		const result = await gunzip.apply(compressed, {});
		expect(bytesToText(result)).toBe('Hello, compressed world!');
	});

	it('detects gzip magic bytes', async () => {
		const compressed = await gzip.apply(textToBytes('test'), {});
		expect(gunzip.detect!(compressed)).toBeGreaterThanOrEqual(0.9);
	});

	it('does not detect non-gzip data', () => {
		expect(gunzip.detect!(textToBytes('not gzip'))).toBeNull();
	});

	it('round-trips', async () => {
		const original = textToBytes('round trip gzip');
		const compressed = await gzip.apply(original, {});
		const decompressed = await gunzip.apply(compressed, {});
		expect(bytesToText(decompressed)).toBe('round trip gzip');
	});

	it('decompresses gzip data with truncated footer via raw inflate fallback', async () => {
		const original = textToBytes('data with missing gzip footer');
		const compressed = await gzip.apply(original, {});
		const truncated = compressed.slice(0, compressed.length - 8);
		const result = await gunzip.apply(truncated, {});
		expect(bytesToText(result)).toBe('data with missing gzip footer');
	});
});

describe('inflate / deflate', () => {
	it('round-trips', async () => {
		const original = textToBytes('round trip deflate');
		const compressed = await deflate.apply(original, {});
		const decompressed = await inflate.apply(compressed, {});
		expect(bytesToText(decompressed)).toBe('round trip deflate');
	});
});

describe('zlib compress / decompress', () => {
	it('round-trips', async () => {
		const original = textToBytes('round trip zlib');
		const compressed = await zlibCompress.apply(original, {});
		const decompressed = await zlibDecompress.apply(compressed, {});
		expect(bytesToText(decompressed)).toBe('round trip zlib');
	});
});
