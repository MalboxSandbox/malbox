import type { TransformDefinition, TransformCategory } from '../types';
import { TransformError } from '../types';

interface WasmTransformWrapper {
	id: string;
	name: string;
	category: TransformCategory;
	init?: () => Promise<void>;
	detect?: (input: Uint8Array) => number | null;
	apply: (
		input: Uint8Array,
		params?: Record<string, string | number | boolean>
	) => Promise<Uint8Array> | Uint8Array;
}

interface ValidationResult {
	valid: boolean;
	errors: string[];
}

export function validateWasmWrapper(obj: unknown): ValidationResult {
	const errors: string[] = [];
	if (!obj || typeof obj !== 'object') {
		return { valid: false, errors: ['Wrapper must be an object'] };
	}
	const w = obj as Record<string, unknown>;
	if (!w.id) errors.push('Wrapper must export an id');
	if (!w.name) errors.push('Wrapper must export a name');
	if (!w.category) errors.push('Wrapper must export a category');
	if (typeof w.apply !== 'function') errors.push('Wrapper must export an apply function');
	return { valid: errors.length === 0, errors };
}

export async function loadWasmTransform(
	wrapper: unknown,
	provenance: 'ui-managed' | 'git-synced' = 'ui-managed'
): Promise<TransformDefinition> {
	const validation = validateWasmWrapper(wrapper);
	if (!validation.valid) {
		throw new Error(`Invalid WASM wrapper: ${validation.errors.join(', ')}`);
	}

	const w = wrapper as WasmTransformWrapper;

	if (w.init) {
		await w.init();
	}

	return {
		id: w.id,
		name: w.name,
		category: w.category,
		provenance,

		detect: w.detect ? (input) => w.detect!(input) : undefined,

		async apply(input, params) {
			try {
				const result = w.apply(input, params);
				return result instanceof Promise ? await result : result;
			} catch (e) {
				if (e instanceof TransformError) throw e;
				throw new TransformError(
					'runtime_error',
					`WASM transform "${w.id}" failed: ${e instanceof Error ? e.message : String(e)}`
				);
			}
		}
	};
}
