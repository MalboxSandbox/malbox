import { describe, it, expect } from 'vitest';
import { loadWasmTransform, validateWasmWrapper } from '../../loader/wasm';

describe('validateWasmWrapper', () => {
	it('accepts valid wrapper with apply', () => {
		const result = validateWasmWrapper({
			id: 'test',
			name: 'Test',
			category: 'compression',
			apply: async () => new Uint8Array()
		});
		expect(result.valid).toBe(true);
	});

	it('accepts wrapper with optional init', () => {
		const result = validateWasmWrapper({
			id: 'test',
			name: 'Test',
			category: 'compression',
			init: async () => {},
			apply: async () => new Uint8Array()
		});
		expect(result.valid).toBe(true);
	});

	it('rejects wrapper without apply', () => {
		const result = validateWasmWrapper({
			id: 'test',
			name: 'Test',
			category: 'compression'
		});
		expect(result.valid).toBe(false);
	});
});

describe('loadWasmTransform', () => {
	it('wraps a valid module', async () => {
		let initCalled = false;
		const wrapper = {
			id: 'wasm-lzma',
			name: 'LZMA',
			category: 'compression' as const,
			async init() {
				initCalled = true;
			},
			async apply(input: Uint8Array) {
				return input;
			}
		};
		const transform = await loadWasmTransform(wrapper);
		expect(transform.id).toBe('wasm-lzma');
		expect(initCalled).toBe(true);
	});

	it('works without init', async () => {
		const wrapper = {
			id: 'wasm-simple',
			name: 'Simple',
			category: 'custom' as const,
			async apply(input: Uint8Array) {
				return new Uint8Array([...input].reverse());
			}
		};
		const transform = await loadWasmTransform(wrapper);
		const result = await transform.apply(new Uint8Array([1, 2]), {});
		expect(result).toEqual(new Uint8Array([2, 1]));
	});
});
