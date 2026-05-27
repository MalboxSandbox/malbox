import { describe, it, expect } from 'vitest';
import { AutoDetect } from '../auto-detect';
import { TransformRegistry } from '../registry';
import { LIMITS } from '../types';
import type { TransformDefinition } from '../types';

function textToBytes(s: string): Uint8Array {
	return new TextEncoder().encode(s);
}

function bytesToText(b: Uint8Array): string {
	return new TextDecoder().decode(b);
}

const base64Decode: TransformDefinition = {
	id: 'base64-decode',
	name: 'Base64 Decode',
	category: 'encoding',
	provenance: 'builtin',

	detect(input) {
		const text = new TextDecoder().decode(input);
		if (/^[A-Za-z0-9+/]+=*$/.test(text) && text.length >= 4 && text.length % 4 === 0) return 0.85;
		return null;
	},
	async apply(input) {
		const text = new TextDecoder().decode(input);
		const binary = atob(text);
		const bytes = new Uint8Array(binary.length);
		for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
		return bytes;
	}
};

const hexDecode: TransformDefinition = {
	id: 'hex-decode',
	name: 'Hex Decode',
	category: 'encoding',
	provenance: 'builtin',

	detect(input) {
		const text = new TextDecoder().decode(input);
		if (/^[0-9a-fA-F]+$/.test(text) && text.length >= 2 && text.length % 2 === 0) return 0.8;
		return null;
	},
	async apply(input) {
		const text = new TextDecoder().decode(input);
		const bytes = new Uint8Array(text.length / 2);
		for (let i = 0; i < text.length; i += 2) {
			bytes[i / 2] = parseInt(text.substring(i, i + 2), 16);
		}
		return bytes;
	}
};

describe('AutoDetect', () => {
	it('returns input unchanged when nothing is detected', async () => {
		const registry = new TransformRegistry();
		const autoDetect = new AutoDetect(registry);
		const input = textToBytes('plain text with spaces');
		const result = await autoDetect.run(input);
		expect(bytesToText(result.output)).toBe('plain text with spaces');
		expect(result.steps).toEqual([]);
	});

	it('detects and applies a single transform', async () => {
		const registry = new TransformRegistry();
		registry.register(base64Decode);
		const autoDetect = new AutoDetect(registry);
		const input = textToBytes(btoa('hello'));
		const result = await autoDetect.run(input);
		expect(bytesToText(result.output)).toBe('hello');
		expect(result.steps.length).toBe(1);
		expect(result.steps[0].transformId).toBe('base64-decode');
		expect(result.steps[0].source).toBe('auto-detect');
	});

	it('chains multiple detected transforms', async () => {
		const registry = new TransformRegistry();
		registry.register(base64Decode);
		const autoDetect = new AutoDetect(registry);
		const input = textToBytes(btoa(btoa('inner')));
		const result = await autoDetect.run(input);
		expect(bytesToText(result.output)).toBe('inner');
		expect(result.steps.length).toBe(2);
	});

	it('picks highest confidence when multiple match', async () => {
		const registry = new TransformRegistry();
		registry.register(base64Decode);
		registry.register(hexDecode);
		const autoDetect = new AutoDetect(registry);
		const input = textToBytes(btoa('test'));
		const result = await autoDetect.run(input);
		expect(result.steps[0].transformId).toBe('base64-decode');
	});

	it('respects confidence threshold', async () => {
		const lowConfidence: TransformDefinition = {
			id: 'low',
			name: 'Low',
			category: 'encoding',
			provenance: 'builtin',

			detect() {
				return 0.5;
			},
			async apply(input) {
				return input;
			}
		};
		const registry = new TransformRegistry();
		registry.register(lowConfidence);
		const autoDetect = new AutoDetect(registry);
		const result = await autoDetect.run(textToBytes('test'));
		expect(result.steps).toEqual([]);
	});

	it('stops at max depth', async () => {
		const alwaysMatch: TransformDefinition = {
			id: 'always',
			name: 'Always',
			category: 'encoding',
			provenance: 'builtin',

			detect() {
				return 0.95;
			},
			async apply(input) {
				return input;
			}
		};
		const registry = new TransformRegistry();
		registry.register(alwaysMatch);
		const autoDetect = new AutoDetect(registry);
		const result = await autoDetect.run(textToBytes('x'));
		expect(result.steps.length).toBe(LIMITS.MAX_AUTO_DETECT_DEPTH);
	});

	it('returns partial results when a chained step throws', async () => {
		const succeedFirst: TransformDefinition = {
			id: 'succeed',
			name: 'Succeed',
			category: 'encoding',
			provenance: 'builtin',

			detect(input) {
				const text = new TextDecoder().decode(input);
				return text === 'step1' ? 0.9 : null;
			},
			async apply() {
				return textToBytes('step2');
			}
		};
		const failSecond: TransformDefinition = {
			id: 'fail-step',
			name: 'Fail Step',
			category: 'compression',
			provenance: 'builtin',

			detect(input) {
				const text = new TextDecoder().decode(input);
				return text === 'step2' ? 0.95 : null;
			},
			async apply() {
				throw new Error('unexpected EOF');
			}
		};
		const registry = new TransformRegistry();
		registry.register(succeedFirst);
		registry.register(failSecond);
		const autoDetect = new AutoDetect(registry);
		const result = await autoDetect.run(textToBytes('step1'));
		expect(result.steps.length).toBe(1);
		expect(result.steps[0].transformId).toBe('succeed');
		expect(bytesToText(result.output)).toBe('step2');
		expect(result.stoppedByError).toBeDefined();
		expect(result.stoppedByError!.transformId).toBe('fail-step');
		expect(result.stoppedByError!.message).toContain('unexpected EOF');
	});

	it('uses lookahead tiebreaker when confidences match', async () => {
		const fakeB64: TransformDefinition = {
			id: 'fake-b64',
			name: 'Fake B64',
			category: 'encoding',
			provenance: 'builtin',

			detect(input) {
				const text = new TextDecoder().decode(input);
				return text === 'AAAA' ? 0.8 : null;
			},
			async apply() {
				return textToBytes('dead-end-no-further-detect');
			}
		};
		const realHex: TransformDefinition = {
			id: 'real-hex',
			name: 'Real Hex',
			category: 'encoding',
			provenance: 'builtin',

			detect(input) {
				const text = new TextDecoder().decode(input);
				if (text === 'AAAA') return 0.8;
				if (input.length === 2 && input[0] === 0xaa && input[1] === 0xaa) return 0.9;
				return null;
			},
			async apply(input) {
				const text = new TextDecoder().decode(input);
				const bytes = new Uint8Array(text.length / 2);
				for (let i = 0; i < text.length; i += 2) {
					bytes[i / 2] = parseInt(text.substring(i, i + 2), 16);
				}
				return bytes;
			}
		};
		const registry = new TransformRegistry();
		registry.register(fakeB64);
		registry.register(realHex);
		const autoDetect = new AutoDetect(registry);
		const result = await autoDetect.run(textToBytes('AAAA'));
		expect(result.steps[0].transformId).toBe('real-hex');
	});
});
