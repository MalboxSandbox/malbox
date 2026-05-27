import type { TransformRegistry } from '../registry';
import { encodingTransforms } from './encoding';
import { compressionTransforms } from './compression';
import { textTransforms } from './text';
import { cryptoTransforms } from './crypto';
import { analysisTransforms } from './analysis';
import { hashTransforms } from './hashes';

export function registerBuiltins(registry: TransformRegistry): void {
	const all = [
		...encodingTransforms,
		...compressionTransforms,
		...textTransforms,
		...cryptoTransforms,
		...analysisTransforms,
		...hashTransforms
	];
	for (const transform of all) {
		registry.register(transform);
	}
}
