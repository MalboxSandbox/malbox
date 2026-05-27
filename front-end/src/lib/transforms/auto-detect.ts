import type { TransformRegistry } from './registry';
import type { PipelineResult, PipelineStep, TransformDefinition } from './types';
import { LIMITS } from './types';

interface DetectCandidate {
	transform: TransformDefinition;
	confidence: number;
}

export interface AutoDetectResult extends PipelineResult {
	stoppedByError?: {
		transformId: string;
		message: string;
	};
}

export class AutoDetect {
	constructor(private registry: TransformRegistry) {}

	async run(input: Uint8Array): Promise<AutoDetectResult> {
		const steps: PipelineStep[] = [];
		const intermediates: Uint8Array[] = [input];
		let current = input;
		let stoppedByError: AutoDetectResult['stoppedByError'];

		for (let depth = 0; depth < LIMITS.MAX_AUTO_DETECT_DEPTH; depth++) {
			const winner = await this.pickBest(current);
			if (!winner) break;

			try {
				const output = await winner.transform.apply(current, {});
				steps.push({
					transformId: winner.transform.id,
					params: {},
					source: 'auto-detect'
				});
				intermediates.push(output);
				current = output;
			} catch (e) {
				stoppedByError = {
					transformId: winner.transform.id,
					message: e instanceof Error ? e.message : String(e)
				};
				break;
			}
		}

		return { output: current, steps, intermediates, stoppedByError };
	}

	peek(input: Uint8Array): { id: string; name: string } | null {
		const best = this.bestCandidate(input);
		return best ? { id: best.transform.id, name: best.transform.name } : null;
	}

	private bestCandidate(input: Uint8Array): DetectCandidate | null {
		const detectable = this.registry.listDetectable();
		let best: DetectCandidate | null = null;
		for (const transform of detectable) {
			const confidence = transform.detect!(input);
			if (confidence !== null && confidence >= LIMITS.AUTO_DETECT_THRESHOLD) {
				if (!best || confidence > best.confidence) {
					best = { transform, confidence };
				}
			}
		}
		return best;
	}

	private async pickBest(input: Uint8Array): Promise<DetectCandidate | null> {
		const detectable = this.registry.listDetectable();
		const candidates: DetectCandidate[] = [];

		for (const transform of detectable) {
			const confidence = transform.detect!(input);
			if (confidence !== null && confidence >= LIMITS.AUTO_DETECT_THRESHOLD) {
				candidates.push({ transform, confidence });
			}
		}

		if (candidates.length === 0) return null;

		candidates.sort((a, b) => b.confidence - a.confidence);

		const best = candidates[0];
		const ties = candidates.filter((c) => c.confidence === best.confidence);

		if (ties.length === 1) return best;

		return this.breakTie(input, ties);
	}

	private async breakTie(
		input: Uint8Array,
		ties: DetectCandidate[]
	): Promise<DetectCandidate | null> {
		const detectable = this.registry.listDetectable();

		for (const candidate of ties) {
			try {
				const output = await candidate.transform.apply(input, {});
				for (const next of detectable) {
					const score = next.detect!(output);
					if (score !== null && score >= LIMITS.AUTO_DETECT_THRESHOLD) {
						return candidate;
					}
				}
			} catch {
				continue;
			}
		}

		return ties[0];
	}
}
