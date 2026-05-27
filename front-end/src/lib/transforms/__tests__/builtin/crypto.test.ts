import { describe, it, expect } from 'vitest';
import { xor, rc4, rot13, rot, slice, bitwiseNot } from '../../builtin/crypto';

function textToBytes(s: string): Uint8Array {
	return new TextEncoder().encode(s);
}

function bytesToText(b: Uint8Array): string {
	return new TextDecoder().decode(b);
}

describe('xor', () => {
	it('xors with single byte key', async () => {
		const input = new Uint8Array([0x41, 0x42, 0x43]);
		const result = await xor.apply(input, { key: '0xFF' });
		expect(result).toEqual(new Uint8Array([0xbe, 0xbd, 0xbc]));
	});

	it('xors with multi-byte key (rolling)', async () => {
		const input = new Uint8Array([0x01, 0x02, 0x03, 0x04]);
		const result = await xor.apply(input, { key: '0x0102' });
		expect(result).toEqual(new Uint8Array([0x00, 0x00, 0x02, 0x06]));
	});

	it('round-trips', async () => {
		const input = textToBytes('secret');
		const encrypted = await xor.apply(input, { key: '0xAB' });
		const decrypted = await xor.apply(encrypted, { key: '0xAB' });
		expect(decrypted).toEqual(input);
	});

	it('throws on missing key', async () => {
		await expect(xor.apply(new Uint8Array([1]), {})).rejects.toThrow();
	});
});

describe('rc4', () => {
	it('encrypts and decrypts (symmetric)', async () => {
		const input = textToBytes('Hello RC4');
		const encrypted = await rc4.apply(input, { key: 'secret' });
		expect(encrypted).not.toEqual(input);
		const decrypted = await rc4.apply(encrypted, { key: 'secret' });
		expect(decrypted).toEqual(input);
	});
});

describe('rot13', () => {
	it('rotates alphabetic characters by 13', async () => {
		expect(bytesToText(await rot13.apply(textToBytes('Hello'), {}))).toBe('Uryyb');
	});

	it('round-trips', async () => {
		const original = textToBytes('Test 123!');
		const rotated = await rot13.apply(original, {});
		const back = await rot13.apply(rotated, {});
		expect(back).toEqual(original);
	});
});

describe('rot', () => {
	it('rotates by custom shift', async () => {
		expect(bytesToText(await rot.apply(textToBytes('abc'), { shift: 1 }))).toBe('bcd');
	});

	it('wraps around', async () => {
		expect(bytesToText(await rot.apply(textToBytes('z'), { shift: 1 }))).toBe('a');
	});
});

describe('slice', () => {
	it('slices from offset with length', async () => {
		const input = new Uint8Array([0, 1, 2, 3, 4, 5]);
		const result = await slice.apply(input, { offset: 2, length: 3 });
		expect(result).toEqual(new Uint8Array([2, 3, 4]));
	});

	it('slices from offset to end when no length', async () => {
		const input = new Uint8Array([0, 1, 2, 3, 4]);
		const result = await slice.apply(input, { offset: 3 });
		expect(result).toEqual(new Uint8Array([3, 4]));
	});
});

describe('bitwiseNot', () => {
	it('inverts all bits', async () => {
		const input = new Uint8Array([0x00, 0xff, 0xaa]);
		const result = await bitwiseNot.apply(input, {});
		expect(result).toEqual(new Uint8Array([0xff, 0x00, 0x55]));
	});

	it('round-trips', async () => {
		const input = new Uint8Array([0xde, 0xad]);
		const notted = await bitwiseNot.apply(input, {});
		const back = await bitwiseNot.apply(notted, {});
		expect(back).toEqual(input);
	});
});
