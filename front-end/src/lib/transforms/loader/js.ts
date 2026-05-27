import type { TransformDefinition, TransformCategory } from '../types';
import { TransformError } from '../types';

interface JsTransformModule {
	id: string;
	name: string;
	category: TransformCategory;
	detect?: (input: Uint8Array) => number | null;
	apply: (
		input: Uint8Array,
		params: Record<string, string | number | boolean>
	) => Promise<Uint8Array> | Uint8Array;
	params?: Record<string, unknown>;
}

interface ValidationResult {
	valid: boolean;
	errors: string[];
}

export function validateJsModule(obj: unknown): ValidationResult {
	const errors: string[] = [];
	if (!obj || typeof obj !== 'object') {
		return { valid: false, errors: ['Module must be an object'] };
	}
	const m = obj as Record<string, unknown>;
	if (!m.id) errors.push('Module must export an id');
	if (!m.name) errors.push('Module must export a name');
	if (!m.category) errors.push('Module must export a category');
	if (typeof m.apply !== 'function') errors.push('Module must export an apply function');
	return { valid: errors.length === 0, errors };
}

export function loadJsTransform(
	module: unknown,
	provenance: 'ui-managed' | 'git-synced' = 'ui-managed'
): TransformDefinition {
	const validation = validateJsModule(module);
	if (!validation.valid) {
		throw new Error(`Invalid JS transform: ${validation.errors.join(', ')}`);
	}

	const m = module as JsTransformModule;

	return {
		id: m.id,
		name: m.name,

		category: m.category,
		provenance,
		detect: m.detect ? (input) => m.detect!(input) : undefined,

		async apply(input, params) {
			try {
				const result = m.apply(input, params);
				return result instanceof Promise ? await result : result;
			} catch (e) {
				if (e instanceof TransformError) throw e;
				throw new TransformError(
					'runtime_error',
					`JS transform "${m.id}" failed: ${e instanceof Error ? e.message : String(e)}`
				);
			}
		}
	};
}
