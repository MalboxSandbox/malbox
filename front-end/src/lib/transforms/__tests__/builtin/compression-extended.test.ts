import { describe, it, expect } from 'vitest';
import { zlibDecompress, zlibCompress } from '../../builtin/compression';

function bytesToText(b: Uint8Array): string {
	return new TextDecoder().decode(b);
}

describe('zlibDecompress', () => {
	it('round-trips with zlibCompress', async () => {
		const input = new TextEncoder().encode('Hello World - zlib test data');
		const compressed = await zlibCompress.apply(input, {});
		expect(compressed.length).toBeLessThan(input.length + 20);
		const decompressed = await zlibDecompress.apply(compressed, {});
		expect(bytesToText(decompressed)).toBe('Hello World - zlib test data');
	});

	it('detects zlib header', () => {
		const score = zlibDecompress.detect!(new Uint8Array([0x78, 0x9c, 0x01, 0x02, 0x03, 0x04]));
		expect(score).toBe(0.8);
	});

	it('rejects non-zlib data', () => {
		const score = zlibDecompress.detect!(new Uint8Array([0x00, 0x00, 0x00, 0x00, 0x00, 0x00]));
		expect(score).toBeNull();
	});

	it('rejects gzip data (different transform)', () => {
		const score = zlibDecompress.detect!(new Uint8Array([0x1f, 0x8b, 0x00, 0x00, 0x00, 0x00]));
		expect(score).toBeNull();
	});
});
