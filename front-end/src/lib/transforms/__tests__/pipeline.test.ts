import { describe, it, expect } from 'vitest';
import { Pipeline } from '../pipeline';
import { TransformRegistry } from '../registry';
import { TransformError, LIMITS } from '../types';
import type { TransformDefinition, PipelineStep } from '../types';

function textToBytes(s: string): Uint8Array {
	return new TextEncoder().encode(s);
}

function bytesToText(b: Uint8Array): string {
	return new TextDecoder().decode(b);
}

function makeRegistry(...transforms: TransformDefinition[]): TransformRegistry {
	const r = new TransformRegistry();
	transforms.forEach((t) => r.register(t));
	return r;
}

const doubler: TransformDefinition = {
	id: 'doubler',
	name: 'Doubler',
	category: 'text',
	provenance: 'builtin',

	async apply(input) {
		const doubled = new Uint8Array(input.length * 2);
		doubled.set(input);
		doubled.set(input, input.length);
		return doubled;
	}
};

const uppercaser: TransformDefinition = {
	id: 'uppercaser',
	name: 'Uppercaser',
	category: 'text',
	provenance: 'builtin',

	async apply(input) {
		return textToBytes(bytesToText(input).toUpperCase());
	}
};

const failingTransform: TransformDefinition = {
	id: 'failer',
	name: 'Failer',
	category: 'text',
	provenance: 'builtin',

	async apply() {
		throw new TransformError('invalid_input', 'always fails');
	}
};

describe('Pipeline', () => {
	it('returns input unchanged for empty pipeline', async () => {
		const pipeline = new Pipeline(makeRegistry());
		const input = textToBytes('hello');
		const result = await pipeline.run(input, []);
		expect(bytesToText(result.output)).toBe('hello');
		expect(result.steps).toEqual([]);
	});

	it('executes a single step', async () => {
		const pipeline = new Pipeline(makeRegistry(uppercaser));
		const steps: PipelineStep[] = [{ transformId: 'uppercaser', params: {}, source: 'manual' }];
		const result = await pipeline.run(textToBytes('hello'), steps);
		expect(bytesToText(result.output)).toBe('HELLO');
	});

	it('chains multiple steps sequentially', async () => {
		const pipeline = new Pipeline(makeRegistry(uppercaser, doubler));
		const steps: PipelineStep[] = [
			{ transformId: 'uppercaser', params: {}, source: 'manual' },
			{ transformId: 'doubler', params: {}, source: 'manual' }
		];
		const result = await pipeline.run(textToBytes('hi'), steps);
		expect(bytesToText(result.output)).toBe('HIHI');
	});

	it('stores intermediates', async () => {
		const pipeline = new Pipeline(makeRegistry(uppercaser, doubler));
		const steps: PipelineStep[] = [
			{ transformId: 'uppercaser', params: {}, source: 'manual' },
			{ transformId: 'doubler', params: {}, source: 'manual' }
		];
		const result = await pipeline.run(textToBytes('hi'), steps);
		expect(result.intermediates.length).toBe(3);
		expect(bytesToText(result.intermediates[0])).toBe('hi');
		expect(bytesToText(result.intermediates[1])).toBe('HI');
		expect(bytesToText(result.intermediates[2])).toBe('HIHI');
	});

	it('fails with not_found for unknown transform', async () => {
		const pipeline = new Pipeline(makeRegistry());
		const steps: PipelineStep[] = [{ transformId: 'nope', params: {}, source: 'manual' }];
		await expect(pipeline.run(textToBytes('x'), steps)).rejects.toThrow(TransformError);
		try {
			await pipeline.run(textToBytes('x'), steps);
		} catch (e) {
			expect((e as TransformError).kind).toBe('not_found');
		}
	});

	it('stops at failed step and reports last good output', async () => {
		const pipeline = new Pipeline(makeRegistry(uppercaser, failingTransform, doubler));
		const steps: PipelineStep[] = [
			{ transformId: 'uppercaser', params: {}, source: 'manual' },
			{ transformId: 'failer', params: {}, source: 'manual' },
			{ transformId: 'doubler', params: {}, source: 'manual' }
		];
		const result = await pipeline.runSafe(textToBytes('hi'), steps);
		expect(result.ok).toBe(false);
		if (!result.ok) {
			expect(bytesToText(result.failure.lastGoodOutput)).toBe('HI');
			expect(result.failure.completedSteps.length).toBe(1);
			expect(result.failure.error.kind).toBe('invalid_input');
		}
	});

	it('rejects pipeline exceeding max depth', async () => {
		const pipeline = new Pipeline(makeRegistry(uppercaser));
		const steps: PipelineStep[] = Array.from({ length: LIMITS.MAX_PIPELINE_DEPTH + 1 }, () => ({
			transformId: 'uppercaser',
			params: {},
			source: 'manual' as const
		}));
		await expect(pipeline.run(textToBytes('x'), steps)).rejects.toThrow(TransformError);
	});

	it('runSafe stores intermediates on success', async () => {
		const pipeline = new Pipeline(makeRegistry(uppercaser, doubler));
		const steps: PipelineStep[] = [
			{ transformId: 'uppercaser', params: {}, source: 'manual' },
			{ transformId: 'doubler', params: {}, source: 'manual' }
		];
		const result = await pipeline.runSafe(textToBytes('hi'), steps);
		expect(result.ok).toBe(true);
		if (result.ok) {
			expect(result.result.intermediates.length).toBe(3);
			expect(bytesToText(result.result.intermediates[0])).toBe('hi');
			expect(bytesToText(result.result.intermediates[1])).toBe('HI');
			expect(bytesToText(result.result.intermediates[2])).toBe('HIHI');
		}
	});
});
