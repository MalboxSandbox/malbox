interface WorkerRequest {
	id: number;
	functionBody: string;
	input: ArrayBuffer;
	params: Record<string, string | number | boolean>;
}

interface WorkerResponse {
	id: number;
	output?: ArrayBuffer;
	error?: string;
}

self.onmessage = async (e: MessageEvent<WorkerRequest>) => {
	const { id, functionBody, input, params } = e.data;
	try {
		const fn = new Function('input', 'params', functionBody);
		const inputBytes = new Uint8Array(input);
		const result = await fn(inputBytes, params);
		const output: ArrayBuffer =
			result instanceof Uint8Array
				? (result.buffer.slice(
						result.byteOffset,
						result.byteOffset + result.byteLength
					) as ArrayBuffer)
				: result;
		const msg: WorkerResponse = { id, output };
		self.postMessage(msg, { transfer: [output] });
	} catch (err) {
		const msg: WorkerResponse = {
			id,
			error: err instanceof Error ? err.message : String(err)
		};
		self.postMessage(msg);
	}
};
