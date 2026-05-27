import { describe, it, expect } from 'vitest';
import { TransformRegistry, Pipeline, AutoDetect, registerBuiltins } from '..';

function textToBytes(s: string): Uint8Array {
	return new TextEncoder().encode(s);
}

function bytesToText(b: Uint8Array): string {
	return new TextDecoder().decode(b);
}

describe('integration', () => {
	it('registers all builtins without error', () => {
		const registry = new TransformRegistry();
		registerBuiltins(registry);
		expect(registry.list().length).toBeGreaterThan(25);
	});

	it('auto-detects base64 encoded content', async () => {
		const registry = new TransformRegistry();
		registerBuiltins(registry);
		const autoDetect = new AutoDetect(registry);
		const encoded = textToBytes(btoa('malware payload'));
		const result = await autoDetect.run(encoded);
		expect(bytesToText(result.output)).toBe('malware payload');
		expect(result.steps[0].transformId).toBe('base64-decode');
	});

	it('runs a manual pipeline: base64 -> hex decode', async () => {
		const registry = new TransformRegistry();
		registerBuiltins(registry);
		const pipeline = new Pipeline(registry);
		const hexPayload = '48656c6c6f';
		const encoded = textToBytes(btoa(hexPayload));
		const result = await pipeline.run(encoded, [
			{ transformId: 'base64-decode', params: {}, source: 'manual' },
			{ transformId: 'hex-decode', params: {}, source: 'manual' }
		]);
		expect(bytesToText(result.output)).toBe('Hello');
	});

	it('lists detectable transforms', () => {
		const registry = new TransformRegistry();
		registerBuiltins(registry);
		const detectable = registry.listDetectable();
		const ids = detectable.map((t) => t.id);
		expect(ids).toContain('base64-decode');
		expect(ids).toContain('gunzip');
		expect(ids).toContain('hex-decode');
		expect(ids).toContain('url-decode');
	});
});
