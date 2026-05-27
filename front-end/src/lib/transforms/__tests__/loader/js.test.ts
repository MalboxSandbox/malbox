import { describe, it, expect } from 'vitest';
import { loadJsTransform, validateJsModule } from '../../loader/js';

describe('validateJsModule', () => {
	it('accepts valid module with apply', () => {
		const result = validateJsModule({
			id: 'test',
			name: 'Test',
			category: 'custom',
			apply: () => new Uint8Array()
		});
		expect(result.valid).toBe(true);
	});

	it('rejects module without apply', () => {
		const result = validateJsModule({
			id: 'test',
			name: 'Test',
			category: 'custom'
		});
		expect(result.valid).toBe(false);
		expect(result.errors).toContain('Module must export an apply function');
	});

	it('rejects module without id', () => {
		const result = validateJsModule({
			name: 'Test',
			category: 'custom',
			apply: () => new Uint8Array()
		});
		expect(result.valid).toBe(false);
	});
});

describe('loadJsTransform', () => {
	it('wraps a valid module object as TransformDefinition', () => {
		const module = {
			id: 'custom-deobf',
			name: 'Deobfuscate',
			category: 'custom' as const,
			detect(input: Uint8Array) {
				return input[0] === 0xde ? 0.9 : null;
			},
			async apply(input: Uint8Array) {
				return input;
			}
		};
		const transform = loadJsTransform(module);
		expect(transform.id).toBe('custom-deobf');
		expect(transform.provenance).toBe('ui-managed');
		expect(typeof transform.detect).toBe('function');
	});

	it('applies correctly', async () => {
		const module = {
			id: 'reverser',
			name: 'Reverser',
			category: 'custom' as const,
			async apply(input: Uint8Array) {
				return new Uint8Array([...input].reverse());
			}
		};
		const transform = loadJsTransform(module);
		const result = await transform.apply(new Uint8Array([1, 2, 3]), {});
		expect(result).toEqual(new Uint8Array([3, 2, 1]));
	});

	it('throws on invalid module', () => {
		expect(() => loadJsTransform({ name: 'bad' } as never)).toThrow();
	});
});
