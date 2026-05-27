import { describe, it, expect } from 'vitest';
import { base32Decode, base32Encode, hexDump } from '../../builtin/encoding';

function textToBytes(s: string): Uint8Array {
	return new TextEncoder().encode(s);
}

function bytesToText(b: Uint8Array): string {
	return new TextDecoder().decode(b);
}

describe('base32Decode', () => {
	it('decodes standard base32', async () => {
		const encoded = textToBytes('JBSWY3DPEBLW64TMMQQQ====');
		const result = bytesToText(await base32Decode.apply(encoded, {}));
		expect(result).toBe('Hello World!');
	});

	it('decodes without padding', async () => {
		const encoded = textToBytes('JBSWY3DPEBLW64TMMQQQ');
		const result = bytesToText(await base32Decode.apply(encoded, {}));
		expect(result).toBe('Hello World!');
	});

	it('handles case insensitivity', async () => {
		const encoded = textToBytes('jbswy3dpeblw64tmmqqq====');
		const result = bytesToText(await base32Decode.apply(encoded, {}));
		expect(result).toBe('Hello World!');
	});

	it('round-trips with base32Encode', async () => {
		const original = textToBytes('malware analysis test data');
		const encoded = await base32Encode.apply(original, {});
		const decoded = await base32Decode.apply(encoded, {});
		expect(bytesToText(decoded)).toBe('malware analysis test data');
	});

	it('detects valid base32 input', () => {
		const encoded = textToBytes('JBSWY3DPEBLW64TMMQ======');
		const score = base32Decode.detect!(encoded);
		expect(score).toBeGreaterThanOrEqual(0.6);
	});

	it('rejects non-base32 input', () => {
		const score = base32Decode.detect!(textToBytes('Hello World!'));
		expect(score).toBeNull();
	});
});

describe('hexDump', () => {
	it('formats bytes as hex dump', async () => {
		const input = textToBytes('Hello, World!');
		const result = bytesToText(await hexDump.apply(input, {}));
		expect(result).toContain('00000000');
		expect(result).toContain('48 65 6c 6c');
		expect(result).toContain('|Hello, World!|');
	});

	it('handles binary data with non-printable bytes', async () => {
		const input = new Uint8Array([0x00, 0x01, 0x02, 0xff]);
		const result = bytesToText(await hexDump.apply(input, {}));
		expect(result).toContain('00 01 02 ff');
		expect(result).toContain('|....|');
	});

	it('handles multi-row data', async () => {
		const input = new Uint8Array(32);
		for (let i = 0; i < 32; i++) input[i] = i;
		const result = bytesToText(await hexDump.apply(input, {}));
		const lines = result.split('\n');
		expect(lines.length).toBe(2);
		expect(lines[0]).toContain('00000000');
		expect(lines[1]).toContain('00000010');
	});

	it('is marked terminal', () => {
		expect(hexDump.terminal).toBe(true);
	});
});
