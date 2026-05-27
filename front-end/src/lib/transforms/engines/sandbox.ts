import { TransformError, LIMITS } from '../types';

interface WorkerResponse {
	id: number;
	output?: ArrayBuffer;
	error?: string;
}

let workerInstance: Worker | null = null;
let nextId = 0;
const pending = new Map<
	number,
	{ resolve: (value: Uint8Array) => void; reject: (reason: Error) => void }
>();

function getWorker(): Worker {
	if (!workerInstance) {
		workerInstance = new Worker(new URL('./sandbox-worker.ts', import.meta.url), {
			type: 'module'
		});
		workerInstance.onmessage = (e: MessageEvent<WorkerResponse>) => {
			const { id, output, error } = e.data;
			const handler = pending.get(id);
			if (!handler) return;
			pending.delete(id);
			if (error) {
				handler.reject(new TransformError('runtime_error', error));
			} else if (output) {
				handler.resolve(new Uint8Array(output));
			} else {
				handler.reject(new TransformError('runtime_error', 'Worker returned no output'));
			}
		};
		workerInstance.onerror = (e) => {
			for (const handler of pending.values()) {
				handler.reject(new TransformError('runtime_error', e.message || 'Worker error'));
			}
			pending.clear();
			workerInstance?.terminate();
			workerInstance = null;
		};
	}
	return workerInstance;
}

export function runInSandbox(
	functionBody: string,
	input: Uint8Array,
	params: Record<string, string | number | boolean>
): Promise<Uint8Array> {
	const id = nextId++;
	const worker = getWorker();
	const inputBuffer = input.buffer.slice(
		input.byteOffset,
		input.byteOffset + input.byteLength
	) as ArrayBuffer;

	return new Promise<Uint8Array>((resolve, reject) => {
		const timer = setTimeout(() => {
			pending.delete(id);
			workerInstance?.terminate();
			workerInstance = null;
			reject(
				new TransformError(
					'timeout',
					`Sandboxed transform exceeded ${LIMITS.EXECUTION_TIMEOUT_MS}ms`
				)
			);
		}, LIMITS.EXECUTION_TIMEOUT_MS);

		pending.set(id, {
			resolve: (value) => {
				clearTimeout(timer);
				resolve(value);
			},
			reject: (reason) => {
				clearTimeout(timer);
				reject(reason);
			}
		});

		worker.postMessage({ id, functionBody, input: inputBuffer, params }, [inputBuffer]);
	});
}
