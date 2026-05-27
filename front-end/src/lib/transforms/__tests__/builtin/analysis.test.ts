import { describe, it, expect } from 'vitest';
import { strings, entropy, magicBytes, frequencyAnalysis } from '../../builtin/analysis';

function textToBytes(s: string): Uint8Array {
	return new TextEncoder().encode(s);
}

function bytesToText(b: Uint8Array): string {
	return new TextDecoder().decode(b);
}

describe('strings', () => {
	it('extracts printable strings with default min length 4', async () => {
		const input = new Uint8Array([
			0x00,
			0x00,
			...textToBytes('hello'),
			0x00,
			0xff,
			0xff,
			...textToBytes('ab'),
			0x00,
			...textToBytes('world!'),
			0x00,
			0x00
		]);
		const result = bytesToText(await strings.apply(input, {}));
		expect(result).toContain('hello');
		expect(result).toContain('world!');
		expect(result).not.toContain('ab');
	});

	it('respects custom min_length param', async () => {
		const input = new Uint8Array([0x00, ...textToBytes('ab'), 0x00, ...textToBytes('cdef'), 0x00]);
		const result = bytesToText(await strings.apply(input, { min_length: 2 }));
		expect(result).toContain('ab');
		expect(result).toContain('cdef');
	});
});

describe('entropy', () => {
	it('returns low entropy for uniform data', async () => {
		const input = new Uint8Array(256).fill(0x41);
		const result = bytesToText(await entropy.apply(input, {}));
		const score = parseFloat(result);
		expect(score).toBeLessThan(1.0);
	});

	it('returns high entropy for random-like data', async () => {
		const input = new Uint8Array(256);
		for (let i = 0; i < 256; i++) input[i] = i;
		const result = bytesToText(await entropy.apply(input, {}));
		const score = parseFloat(result);
		expect(score).toBeGreaterThan(7.0);
	});

	it('is marked terminal', () => {
		expect(entropy.terminal).toBe(true);
	});
});

describe('magicBytes', () => {
	it('identifies gzip', async () => {
		const input = new Uint8Array([0x1f, 0x8b, 0x08, 0x00]);
		const result = bytesToText(await magicBytes.apply(input, {}));
		expect(result.toLowerCase()).toContain('gzip');
	});

	it('identifies PNG', async () => {
		const input = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
		const result = bytesToText(await magicBytes.apply(input, {}));
		expect(result.toLowerCase()).toContain('png');
	});

	it('returns unknown for unrecognized data', async () => {
		const input = new Uint8Array([0x01, 0x02, 0x03]);
		const result = bytesToText(await magicBytes.apply(input, {}));
		expect(result.toLowerCase()).toContain('unknown');
	});

	it('is marked terminal', () => {
		expect(magicBytes.terminal).toBe(true);
	});
});

describe('frequencyAnalysis', () => {
	it('returns JSON with byte frequencies', async () => {
		const input = new Uint8Array([0x41, 0x41, 0x42]);
		const result = JSON.parse(bytesToText(await frequencyAnalysis.apply(input, {})));
		expect(result['0x41']).toBe(2);
		expect(result['0x42']).toBe(1);
	});

	it('is marked terminal', () => {
		expect(frequencyAnalysis.terminal).toBe(true);
	});
});
