import { describe, it, expect } from 'vitest';
import {
	utf16leDecode,
	utf16beDecode,
	regexExtract,
	regexReplace,
	split,
	reverse
} from '../../builtin/text';

function textToBytes(s: string): Uint8Array {
	return new TextEncoder().encode(s);
}

function bytesToText(b: Uint8Array): string {
	return new TextDecoder().decode(b);
}

describe('utf16leDecode', () => {
	it('decodes utf-16 LE with BOM', async () => {
		const bytes = new Uint8Array([0xff, 0xfe, 0x48, 0x00, 0x69, 0x00]);
		const result = await utf16leDecode.apply(bytes, {});
		expect(bytesToText(result)).toBe('Hi');
	});

	it('detects utf-16 LE BOM', () => {
		const bytes = new Uint8Array([0xff, 0xfe, 0x48, 0x00]);
		expect(utf16leDecode.detect!(bytes)).toBeGreaterThanOrEqual(0.7);
	});
});

describe('utf16beDecode', () => {
	it('decodes utf-16 BE with BOM', async () => {
		const bytes = new Uint8Array([0xfe, 0xff, 0x00, 0x48, 0x00, 0x69]);
		const result = await utf16beDecode.apply(bytes, {});
		expect(bytesToText(result)).toBe('Hi');
	});
});

describe('regexExtract', () => {
	it('extracts matches joined by newline', async () => {
		const input = textToBytes('Call 192.168.1.1 or 10.0.0.1 for info');
		const result = await regexExtract.apply(input, {
			pattern: '\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}'
		});
		expect(bytesToText(result)).toBe('192.168.1.1\n10.0.0.1');
	});

	it('throws on missing pattern param', async () => {
		await expect(regexExtract.apply(textToBytes('test'), {})).rejects.toThrow();
	});
});

describe('regexReplace', () => {
	it('replaces matches', async () => {
		const input = textToBytes('foo123bar456');
		const result = await regexReplace.apply(input, { pattern: '\\d+', replacement: 'NUM' });
		expect(bytesToText(result)).toBe('fooNUMbarNUM');
	});
});

describe('split', () => {
	it('splits by delimiter and returns lines', async () => {
		const input = textToBytes('a,b,c');
		const result = await split.apply(input, { delimiter: ',' });
		expect(bytesToText(result)).toBe('a\nb\nc');
	});
});

describe('reverse', () => {
	it('reverses bytes', async () => {
		const input = new Uint8Array([1, 2, 3, 4]);
		const result = await reverse.apply(input, {});
		expect(result).toEqual(new Uint8Array([4, 3, 2, 1]));
	});
});
