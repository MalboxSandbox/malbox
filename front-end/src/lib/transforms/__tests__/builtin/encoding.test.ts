import { describe, it, expect } from 'vitest';
import {
	base64Decode,
	base64Encode,
	hexDecode,
	hexEncode,
	urlDecode,
	urlEncode,
	htmlEntityDecode,
	ascii85Decode,
	quotedPrintableDecode
} from '../../builtin/encoding';

function textToBytes(s: string): Uint8Array {
	return new TextEncoder().encode(s);
}

function bytesToText(b: Uint8Array): string {
	return new TextDecoder().decode(b);
}

async function applyText(
	t: {
		apply: (i: Uint8Array, p: Record<string, string | number | boolean>) => Promise<Uint8Array>;
	},
	input: string
): Promise<string> {
	return bytesToText(await t.apply(textToBytes(input), {}));
}

describe('base64', () => {
	it('decodes standard base64', async () => {
		expect(await applyText(base64Decode, btoa('Hello, World!'))).toBe('Hello, World!');
	});

	it('decodes url-safe base64', async () => {
		const urlSafe = btoa('\xff\xfe\xfd').replace(/\+/g, '-').replace(/\//g, '_');
		const result = await base64Decode.apply(textToBytes(urlSafe), {});
		expect(result).toEqual(new Uint8Array([0xff, 0xfe, 0xfd]));
	});

	it('detects base64 with high confidence', () => {
		expect(base64Decode.detect!(textToBytes(btoa('test data here')))).toBeGreaterThanOrEqual(0.7);
	});

	it('does not detect plain text', () => {
		const score = base64Decode.detect!(textToBytes('hello world with spaces'));
		expect(score === null || score < 0.7).toBe(true);
	});

	it('round-trips', async () => {
		const original = textToBytes('round trip test');
		const encoded = await base64Encode.apply(original, {});
		const decoded = await base64Decode.apply(encoded, {});
		expect(decoded).toEqual(original);
	});

	it('encode is inverse of decode', () => {
		expect(base64Decode.inverse).toBe('base64-encode');
		expect(base64Encode.inverse).toBe('base64-decode');
	});
});

describe('hex', () => {
	it('decodes hex string', async () => {
		const result = await hexDecode.apply(textToBytes('48656c6c6f'), {});
		expect(bytesToText(result)).toBe('Hello');
	});

	it('handles uppercase hex', async () => {
		const result = await hexDecode.apply(textToBytes('48656C6C6F'), {});
		expect(bytesToText(result)).toBe('Hello');
	});

	it('detects hex with high confidence', () => {
		expect(hexDecode.detect!(textToBytes('48656c6c6f'))).toBeGreaterThanOrEqual(0.7);
	});

	it('round-trips', async () => {
		const original = new Uint8Array([0xde, 0xad, 0xbe, 0xef]);
		const encoded = await hexEncode.apply(original, {});
		const decoded = await hexDecode.apply(encoded, {});
		expect(decoded).toEqual(original);
	});
});

describe('url', () => {
	it('decodes url encoding', async () => {
		expect(await applyText(urlDecode, 'hello%20world%21')).toBe('hello world!');
	});

	it('detects url encoding', () => {
		expect(urlDecode.detect!(textToBytes('hello%20world'))).toBeGreaterThanOrEqual(0.7);
	});

	it('round-trips', async () => {
		const original = textToBytes('hello world!');
		const encoded = await urlEncode.apply(original, {});
		const decoded = await urlDecode.apply(encoded, {});
		expect(decoded).toEqual(original);
	});
});

describe('html entity', () => {
	it('decodes named entities', async () => {
		expect(await applyText(htmlEntityDecode, '&lt;div&gt;&amp;&quot;')).toBe('<div>&"');
	});

	it('decodes numeric entities', async () => {
		expect(await applyText(htmlEntityDecode, '&#60;&#x3E;')).toBe('<>');
	});

	it('detects html entities', () => {
		expect(htmlEntityDecode.detect!(textToBytes('&lt;script&gt;'))).toBeGreaterThanOrEqual(0.7);
	});
});

describe('ascii85', () => {
	it('decodes ascii85', async () => {
		const result = await ascii85Decode.apply(textToBytes('<~87cURD]j7BEbo80~>'), {});
		expect(bytesToText(result)).toBe('Hello world!');
	});

	it('detects ascii85 delimiters', () => {
		expect(ascii85Decode.detect!(textToBytes('<~87cURD]j7BEbo80~>'))).toBeGreaterThanOrEqual(0.7);
	});
});

describe('quoted-printable', () => {
	it('decodes quoted-printable', async () => {
		expect(await applyText(quotedPrintableDecode, 'hello=20world=0D=0A')).toBe('hello world\r\n');
	});

	it('handles soft line breaks', async () => {
		expect(await applyText(quotedPrintableDecode, 'long=\r\nline')).toBe('longline');
	});
});
