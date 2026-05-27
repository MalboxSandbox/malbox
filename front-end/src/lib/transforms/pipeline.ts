import type { TransformRegistry } from './registry';
import type {
	PipelineStep,
	PipelineResult,
	PipelineFailure,
	TransformDefinition,
	TransformContext
} from './types';
import { TransformError, LIMITS } from './types';

type SafeResult = { ok: true; result: PipelineResult } | { ok: false; failure: PipelineFailure };

export interface PipelineOptions {
	filename?: string;
}

function applyWithTimeout(
	transform: TransformDefinition,
	input: Uint8Array,
	params: Record<string, string | number | boolean>,
	context: TransformContext
): Promise<Uint8Array> {
	const work = transform.apply(input, params, context);
	if (LIMITS.EXECUTION_TIMEOUT_MS <= 0) return work;

	return Promise.race([
		work,
		new Promise<never>((_, reject) => {
			setTimeout(
				() =>
					reject(
						new TransformError(
							'timeout',
							`Step ${context.stepIndex} ("${transform.id}") exceeded ${LIMITS.EXECUTION_TIMEOUT_MS}ms`,
							context.stepIndex
						)
					),
				LIMITS.EXECUTION_TIMEOUT_MS
			);
		})
	]);
}

export class Pipeline {
	constructor(private registry: TransformRegistry) {}

	async run(
		input: Uint8Array,
		steps: PipelineStep[],
		options?: PipelineOptions
	): Promise<PipelineResult> {
		if (steps.length > LIMITS.MAX_PIPELINE_DEPTH) {
			throw new TransformError(
				'depth_exceeded',
				`Pipeline has ${steps.length} steps, max is ${LIMITS.MAX_PIPELINE_DEPTH}`
			);
		}

		const intermediates: Uint8Array[] = [input];
		let current = input;

		for (let i = 0; i < steps.length; i++) {
			const step = steps[i];
			const transform = this.registry.get(step.transformId);
			if (!transform) {
				throw new TransformError('not_found', `Transform "${step.transformId}" not found`, i);
			}

			const ctx: TransformContext = {
				stepIndex: i,
				totalSteps: steps.length,
				filename: options?.filename,
				originalSize: input.byteLength
			};
			const output = await applyWithTimeout(transform, current, step.params, ctx);

			if (output.byteLength > LIMITS.MAX_OUTPUT_BYTES) {
				throw new TransformError(
					'output_too_large',
					`Step ${i} output is ${output.byteLength} bytes, max is ${LIMITS.MAX_OUTPUT_BYTES}`,
					i
				);
			}

			intermediates.push(output);
			current = output;
		}

		return { output: current, steps: [...steps], intermediates };
	}

	async runSafe(input: Uint8Array, steps: PipelineStep[]): Promise<SafeResult> {
		if (steps.length > LIMITS.MAX_PIPELINE_DEPTH) {
			return {
				ok: false,
				failure: {
					error: new TransformError(
						'depth_exceeded',
						`Pipeline has ${steps.length} steps, max is ${LIMITS.MAX_PIPELINE_DEPTH}`
					),
					lastGoodOutput: input,
					completedSteps: [],
					failedStep: steps[0]
				}
			};
		}

		const completedSteps: PipelineStep[] = [];
		const intermediates: Uint8Array[] = [input];
		let current = input;

		for (let i = 0; i < steps.length; i++) {
			const step = steps[i];
			const transform = this.registry.get(step.transformId);
			if (!transform) {
				return {
					ok: false,
					failure: {
						error: new TransformError('not_found', `Transform "${step.transformId}" not found`, i),
						lastGoodOutput: current,
						completedSteps: [...completedSteps],
						failedStep: step
					}
				};
			}

			try {
				const ctx: TransformContext = {
					stepIndex: i,
					totalSteps: steps.length,
					originalSize: input.byteLength
				};
				const output = await applyWithTimeout(transform, current, step.params, ctx);

				if (output.byteLength > LIMITS.MAX_OUTPUT_BYTES) {
					return {
						ok: false,
						failure: {
							error: new TransformError(
								'output_too_large',
								`Step ${i} output is ${output.byteLength} bytes`,
								i
							),
							lastGoodOutput: current,
							completedSteps: [...completedSteps],
							failedStep: step
						}
					};
				}

				intermediates.push(output);
				current = output;
				completedSteps.push(step);
			} catch (e) {
				const error =
					e instanceof TransformError
						? e
						: new TransformError('runtime_error', e instanceof Error ? e.message : String(e), i);
				return {
					ok: false,
					failure: {
						error,
						lastGoodOutput: current,
						completedSteps: [...completedSteps],
						failedStep: step
					}
				};
			}
		}

		return {
			ok: true,
			result: { output: current, steps: [...completedSteps], intermediates }
		};
	}
}
