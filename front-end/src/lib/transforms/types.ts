export type TransformCategory =
	| 'encoding'
	| 'compression'
	| 'crypto'
	| 'text'
	| 'analysis'
	| 'custom';

export type TransformProvenance = 'builtin' | 'ui-managed' | 'git-synced';

export interface TransformParamSchema {
	key: string;
	label: string;
	type: 'string' | 'number' | 'boolean' | 'select';
	required: boolean;
	default?: string | number | boolean;
	options?: string[];
	description?: string;
}

export interface TransformContext {
	stepIndex: number;
	totalSteps: number;
	filename?: string;
	originalSize?: number;
}

export interface TransformDefinition {
	id: string;
	name: string;
	category: TransformCategory;
	provenance: TransformProvenance;
	paramSchema?: TransformParamSchema[];
	inverse?: string;
	terminal?: boolean;
	description?: string;

	detect?(input: Uint8Array): number | null;
	apply(
		input: Uint8Array,
		params: Record<string, string | number | boolean>,
		context?: TransformContext
	): Promise<Uint8Array>;
}

export interface PipelineStep {
	transformId: string;
	params: Record<string, string | number | boolean>;
	source: 'auto-detect' | 'manual';
}

export interface PipelineResult {
	output: Uint8Array;
	steps: PipelineStep[];
	intermediates: Uint8Array[];
}

export type TransformErrorKind =
	| 'invalid_input'
	| 'param_error'
	| 'runtime_error'
	| 'not_found'
	| 'timeout'
	| 'output_too_large'
	| 'depth_exceeded';

export class TransformError extends Error {
	constructor(
		public readonly kind: TransformErrorKind,
		message: string,
		public readonly stepIndex?: number
	) {
		super(message);
		this.name = 'TransformError';
	}
}

export interface PipelineFailure {
	error: TransformError;
	lastGoodOutput: Uint8Array;
	completedSteps: PipelineStep[];
	failedStep: PipelineStep;
}

export const LIMITS = {
	MAX_OUTPUT_BYTES: 50 * 1024 * 1024,
	EXECUTION_TIMEOUT_MS: 10_000,
	MAX_PIPELINE_DEPTH: 50,
	MAX_AUTO_DETECT_DEPTH: 10,
	AUTO_DETECT_THRESHOLD: 0.7
} as const;
