import { describe, it, expect } from 'vitest';
import { TransformRegistry } from '../registry';
import type { TransformDefinition } from '../types';

function makeTransform(
	id: string,
	provenance: 'builtin' | 'ui-managed' | 'git-synced' = 'builtin'
): TransformDefinition {
	return {
		id,
		name: id,
		category: 'encoding',
		provenance,

		async apply(input) {
			return input;
		}
	};
}

describe('TransformRegistry', () => {
	it('registers and retrieves a transform', () => {
		const registry = new TransformRegistry();
		const t = makeTransform('base64-decode');
		registry.register(t);
		expect(registry.get('base64-decode')).toBe(t);
	});

	it('returns undefined for unknown id', () => {
		const registry = new TransformRegistry();
		expect(registry.get('nonexistent')).toBeUndefined();
	});

	it('lists all registered transforms', () => {
		const registry = new TransformRegistry();
		registry.register(makeTransform('a'));
		registry.register(makeTransform('b'));
		expect(
			registry
				.list()
				.map((t) => t.id)
				.sort()
		).toEqual(['a', 'b']);
	});

	it('filters by category', () => {
		const registry = new TransformRegistry();
		const enc = makeTransform('a');
		enc.category = 'encoding';
		const comp = makeTransform('b');
		comp.category = 'compression';
		registry.register(enc);
		registry.register(comp);
		expect(registry.list('compression').map((t) => t.id)).toEqual(['b']);
	});

	it('ui-managed overrides builtin with same id', () => {
		const registry = new TransformRegistry();
		registry.register(makeTransform('xor', 'builtin'));
		registry.register(makeTransform('xor', 'ui-managed'));
		expect(registry.get('xor')!.provenance).toBe('ui-managed');
	});

	it('ui-managed overrides git-synced with same id', () => {
		const registry = new TransformRegistry();
		registry.register(makeTransform('xor', 'git-synced'));
		registry.register(makeTransform('xor', 'ui-managed'));
		expect(registry.get('xor')!.provenance).toBe('ui-managed');
	});

	it('git-synced overrides builtin with same id', () => {
		const registry = new TransformRegistry();
		registry.register(makeTransform('xor', 'builtin'));
		registry.register(makeTransform('xor', 'git-synced'));
		expect(registry.get('xor')!.provenance).toBe('git-synced');
	});

	it('builtin does not override git-synced', () => {
		const registry = new TransformRegistry();
		registry.register(makeTransform('xor', 'git-synced'));
		registry.register(makeTransform('xor', 'builtin'));
		expect(registry.get('xor')!.provenance).toBe('git-synced');
	});

	it('unregisters a transform', () => {
		const registry = new TransformRegistry();
		registry.register(makeTransform('a'));
		registry.unregister('a');
		expect(registry.get('a')).toBeUndefined();
	});

	it('lists only transforms with detect()', () => {
		const registry = new TransformRegistry();
		const detectable = makeTransform('a');
		detectable.detect = () => 0.9;
		registry.register(detectable);
		registry.register(makeTransform('b'));
		expect(registry.listDetectable().map((t) => t.id)).toEqual(['a']);
	});
});
