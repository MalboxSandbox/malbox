import { describe, it, expect } from 'vitest';
import { xorBruteForce, xorKeySearch } from '../../builtin/crypto';

function textToBytes(s: string): Uint8Array {
	return new TextEncoder().encode(s);
}

function bytesToText(b: Uint8Array): string {
	return new TextDecoder().decode(b);
}

function xorBytes(data: Uint8Array, key: number): Uint8Array {
	const out = new Uint8Array(data.length);
	for (let i = 0; i < data.length; i++) out[i] = data[i] ^ key;
	return out;
}

describe('xorBruteForce', () => {
	it('produces output sorted by entropy and includes preview', async () => {
		const plaintext = textToBytes('AAAA');
		const encrypted = xorBytes(plaintext, 0x20);

		const result = bytesToText(await xorBruteForce.apply(encrypted, { top_n: 3 }));
		const lines = result.split('\n');
		expect(lines.length).toBe(3);
		expect(lines[0]).toContain('Key:');
		expect(lines[0]).toContain('Entropy:');
		expect(lines[0]).toContain('Printable:');
		expect(lines[0]).toContain('Preview:');
	});

	it('ranks results by entropy (lowest first)', async () => {
		const plaintext = textToBytes('AAAAAAAAAAAAAAAAAAAAAAAAAAAA');
		const encrypted = xorBytes(plaintext, 0x10);
		const result = bytesToText(await xorBruteForce.apply(encrypted, { top_n: 3 }));
		const lines = result.split('\n');
		const entropies = lines.map((l) => {
			const match = l.match(/Entropy: ([\d.]+)/);
			return match ? parseFloat(match[1]) : Infinity;
		});
		for (let i = 1; i < entropies.length; i++) {
			expect(entropies[i]).toBeGreaterThanOrEqual(entropies[i - 1]);
		}
	});

	it('includes printable ratio in output', async () => {
		const plaintext = textToBytes('Hello, World!');
		const encrypted = xorBytes(plaintext, 0xff);
		const result = bytesToText(await xorBruteForce.apply(encrypted, {}));
		expect(result).toContain('Printable:');
	});

	it('skips key 0x00', async () => {
		const data = textToBytes('test');
		const result = bytesToText(await xorBruteForce.apply(data, { top_n: 255 }));
		expect(result).not.toContain('Key: 0x00');
	});

	it('is marked terminal', () => {
		expect(xorBruteForce.terminal).toBe(true);
	});
});

describe('xorKeySearch (known-plaintext)', () => {
	it('recovers data using known plaintext "MZ"', async () => {
		const plaintext = new Uint8Array([0x4d, 0x5a, 0x90, 0x00, 0x03, 0x00]);
		const key = 0xab;
		const encrypted = xorBytes(plaintext, key);

		const result = await xorKeySearch.apply(encrypted, { known: 'MZ' });
		expect(result[0]).toBe(0x4d);
		expect(result[1]).toBe(0x5a);
	});

	it('derives multi-byte key from longer known plaintext', async () => {
		const key = new Uint8Array([0xde, 0xad]);
		const plaintext = textToBytes('This program cannot be run in DOS mode');
		const encrypted = new Uint8Array(plaintext.length);
		for (let i = 0; i < plaintext.length; i++) {
			encrypted[i] = plaintext[i] ^ key[i % key.length];
		}

		const result = await xorKeySearch.apply(encrypted, { known: 'This program' });
		const decoded = bytesToText(result);
		expect(decoded).toContain('This program cannot be run in DOS mode');
	});

	it('throws on missing known param', async () => {
		await expect(xorKeySearch.apply(textToBytes('test'), {})).rejects.toThrow(
			'Missing required parameter: known'
		);
	});
});
