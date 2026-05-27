import { describe, it, expect } from 'vitest';
import { Pipeline } from '../pipeline';
import { TransformRegistry } from '../registry';
import { registerBuiltins } from '../builtin';

function textToBytes(s: string): Uint8Array {
	return new TextEncoder().encode(s);
}

function bytesToText(b: Uint8Array): string {
	return new TextDecoder().decode(b);
}

function makeRegistryWithBuiltins(): TransformRegistry {
	const r = new TransformRegistry();
	registerBuiltins(r);
	return r;
}

describe('Pipeline with parameterized transforms', () => {
	it('passes params through XOR then base64 encode', async () => {
		const registry = makeRegistryWithBuiltins();
		const pipeline = new Pipeline(registry);

		const input = textToBytes('AAAA');
		const result = await pipeline.run(input, [
			{ transformId: 'xor', params: { key: '0x20' }, source: 'manual' },
			{ transformId: 'base64-encode', params: {}, source: 'manual' }
		]);

		const xored = new Uint8Array([0x41 ^ 0x20, 0x41 ^ 0x20, 0x41 ^ 0x20, 0x41 ^ 0x20]);
		expect(result.intermediates[1]).toEqual(xored);
		expect(result.steps.length).toBe(2);
	});

	it('passes ROT-N shift param correctly', async () => {
		const registry = makeRegistryWithBuiltins();
		const pipeline = new Pipeline(registry);

		const result = await pipeline.run(textToBytes('abc'), [
			{ transformId: 'rot', params: { shift: 3 }, source: 'manual' }
		]);
		expect(bytesToText(result.output)).toBe('def');
	});

	it('chains slice + hex-encode with params', async () => {
		const registry = makeRegistryWithBuiltins();
		const pipeline = new Pipeline(registry);

		const input = new Uint8Array([0xde, 0xad, 0xbe, 0xef]);
		const result = await pipeline.run(input, [
			{ transformId: 'slice', params: { offset: 1, length: 2 }, source: 'manual' },
			{ transformId: 'hex-encode', params: {}, source: 'manual' }
		]);
		expect(bytesToText(result.output)).toBe('adbe');
	});
});

describe('Pipeline with TransformContext', () => {
	it('provides context to transforms', async () => {
		const registry = makeRegistryWithBuiltins();
		const pipeline = new Pipeline(registry);

		const result = await pipeline.run(
			textToBytes('test'),
			[{ transformId: 'hex-encode', params: {}, source: 'manual' }],
			{ filename: 'test.bin' }
		);

		expect(bytesToText(result.output)).toBe('74657374');
	});
});

describe('Pipeline with large input', () => {
	it('handles 1MB input', async () => {
		const registry = makeRegistryWithBuiltins();
		const pipeline = new Pipeline(registry);

		const input = new Uint8Array(1024 * 1024);
		for (let i = 0; i < input.length; i++) input[i] = i & 0xff;

		const result = await pipeline.run(input, [
			{ transformId: 'hex-encode', params: {}, source: 'manual' }
		]);

		expect(result.output.length).toBe(input.length * 2);
	});
});

describe('Regex safety', () => {
	it('rejects catastrophic backtracking patterns', async () => {
		const registry = makeRegistryWithBuiltins();
		const transform = registry.get('regex-extract')!;

		await expect(transform.apply(textToBytes('aaaaaa'), { pattern: '(a+)+$' })).rejects.toThrow(
			'catastrophic'
		);
	});

	it('rejects invalid regex syntax', async () => {
		const registry = makeRegistryWithBuiltins();
		const transform = registry.get('regex-extract')!;

		await expect(transform.apply(textToBytes('test'), { pattern: '[invalid' })).rejects.toThrow(
			'Invalid regex'
		);
	});

	it('allows safe regex patterns', async () => {
		const registry = makeRegistryWithBuiltins();
		const transform = registry.get('regex-extract')!;

		const result = await transform.apply(textToBytes('foo@bar.com test@example.org'), {
			pattern: '[\\w.]+@[\\w.]+'
		});
		const text = bytesToText(result);
		expect(text).toContain('foo@bar.com');
		expect(text).toContain('test@example.org');
	});
});
