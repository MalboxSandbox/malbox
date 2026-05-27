import { describe, it, expect } from 'vitest';
import { md5Hash, sha1Hash, sha256Hash, sha512Hash, allHashes } from '../../builtin/hashes';

function textToBytes(s: string): Uint8Array {
	return new TextEncoder().encode(s);
}

function bytesToText(b: Uint8Array): string {
	return new TextDecoder().decode(b);
}

describe('md5Hash', () => {
	it('hashes empty input', async () => {
		const result = bytesToText(await md5Hash.apply(new Uint8Array(0), {}));
		expect(result).toBe('d41d8cd98f00b204e9800998ecf8427e');
	});

	it('hashes "hello"', async () => {
		const result = bytesToText(await md5Hash.apply(textToBytes('hello'), {}));
		expect(result).toBe('5d41402abc4b2a76b9719d911017c592');
	});

	it('hashes "The quick brown fox jumps over the lazy dog"', async () => {
		const result = bytesToText(
			await md5Hash.apply(textToBytes('The quick brown fox jumps over the lazy dog'), {})
		);
		expect(result).toBe('9e107d9d372bb6826bd81d3542a419d6');
	});

	it('is marked terminal', () => {
		expect(md5Hash.terminal).toBe(true);
	});
});

describe('sha1Hash', () => {
	it('hashes empty input', async () => {
		const result = bytesToText(await sha1Hash.apply(new Uint8Array(0), {}));
		expect(result).toBe('da39a3ee5e6b4b0d3255bfef95601890afd80709');
	});

	it('hashes "hello"', async () => {
		const result = bytesToText(await sha1Hash.apply(textToBytes('hello'), {}));
		expect(result).toBe('aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d');
	});

	it('is marked terminal', () => {
		expect(sha1Hash.terminal).toBe(true);
	});
});

describe('sha256Hash', () => {
	it('hashes empty input', async () => {
		const result = bytesToText(await sha256Hash.apply(new Uint8Array(0), {}));
		expect(result).toBe('e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855');
	});

	it('hashes "hello"', async () => {
		const result = bytesToText(await sha256Hash.apply(textToBytes('hello'), {}));
		expect(result).toBe('2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824');
	});

	it('is marked terminal', () => {
		expect(sha256Hash.terminal).toBe(true);
	});
});

describe('sha512Hash', () => {
	it('hashes "hello"', async () => {
		const result = bytesToText(await sha512Hash.apply(textToBytes('hello'), {}));
		expect(result).toBe(
			'9b71d224bd62f3785d96d46ad3ea3d73319bfbc2890caadae2dff72519673ca72323c3d99ba5c11d7c7acc6e14b8c5da0c4663475c2e5c3adef46f73bcdec043'
		);
	});

	it('is marked terminal', () => {
		expect(sha512Hash.terminal).toBe(true);
	});
});

describe('allHashes', () => {
	it('includes all four hash types', async () => {
		const result = bytesToText(await allHashes.apply(textToBytes('test'), {}));
		expect(result).toContain('MD5:');
		expect(result).toContain('SHA-1:');
		expect(result).toContain('SHA-256:');
		expect(result).toContain('SHA-512:');
	});

	it('is marked terminal', () => {
		expect(allHashes.terminal).toBe(true);
	});
});
