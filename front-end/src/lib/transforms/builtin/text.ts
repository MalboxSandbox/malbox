import type { TransformDefinition } from '../types';
import { TransformError } from '../types';

function validateRegex(pattern: string): void {
	try {
		new RegExp(pattern);
	} catch (e) {
		throw new TransformError(
			'param_error',
			`Invalid regex: ${e instanceof Error ? e.message : String(e)}`
		);
	}
	if (/(\+|\*|\{[\d,]+\})\)(\+|\*|\{[\d,]+\})/.test(pattern)) {
		throw new TransformError(
			'param_error',
			'Potentially catastrophic regex detected (nested quantifiers). Simplify the pattern.'
		);
	}
}

export const utf8Decode: TransformDefinition = {
	id: 'utf8-decode',
	name: 'UTF-8 Decode',
	category: 'text',
	provenance: 'builtin',

	async apply(input) {
		const text = new TextDecoder('utf-8', { fatal: true }).decode(input);
		return new TextEncoder().encode(text);
	}
};

export const utf16leDecode: TransformDefinition = {
	id: 'utf16le-decode',
	name: 'UTF-16 LE Decode',
	category: 'text',
	provenance: 'builtin',

	detect(input) {
		if (input.length >= 2 && input[0] === 0xff && input[1] === 0xfe) return 0.9;
		if (input.length >= 4 && input.length % 2 === 0) {
			let nullCount = 0;
			for (let i = 1; i < input.length; i += 2) {
				if (input[i] === 0) nullCount++;
			}
			if (nullCount / (input.length / 2) > 0.5) return 0.75;
		}
		return null;
	},

	async apply(input) {
		let data = input;
		if (data.length >= 2 && data[0] === 0xff && data[1] === 0xfe) {
			data = data.slice(2);
		}
		const text = new TextDecoder('utf-16le').decode(data);
		return new TextEncoder().encode(text);
	}
};

export const utf16beDecode: TransformDefinition = {
	id: 'utf16be-decode',
	name: 'UTF-16 BE Decode',
	category: 'text',
	provenance: 'builtin',

	detect(input) {
		if (input.length >= 2 && input[0] === 0xfe && input[1] === 0xff) return 0.9;
		return null;
	},

	async apply(input) {
		let data = input;
		if (data.length >= 2 && data[0] === 0xfe && data[1] === 0xff) {
			data = data.slice(2);
		}
		const text = new TextDecoder('utf-16be').decode(data);
		return new TextEncoder().encode(text);
	}
};

export const regexExtract: TransformDefinition = {
	id: 'regex-extract',
	name: 'Regex Extract',
	category: 'text',
	provenance: 'builtin',

	paramSchema: [
		{
			key: 'pattern',
			label: 'Pattern',
			type: 'string',
			required: true,
			description: 'Regular expression pattern'
		}
	],

	async apply(input, params) {
		const pattern = params.pattern;
		if (!pattern || typeof pattern !== 'string') {
			throw new TransformError('param_error', 'Missing required parameter: pattern');
		}
		validateRegex(String(pattern));
		const text = new TextDecoder().decode(input);
		const matches = text.match(new RegExp(String(pattern), 'g'));
		return new TextEncoder().encode(matches ? matches.join('\n') : '');
	}
};

export const regexReplace: TransformDefinition = {
	id: 'regex-replace',
	name: 'Regex Replace',
	category: 'text',
	provenance: 'builtin',

	paramSchema: [
		{ key: 'pattern', label: 'Pattern', type: 'string', required: true },
		{ key: 'replacement', label: 'Replacement', type: 'string', required: true }
	],

	async apply(input, params) {
		if (!params.pattern || typeof params.pattern !== 'string') {
			throw new TransformError('param_error', 'Missing required parameter: pattern');
		}
		validateRegex(String(params.pattern));
		const text = new TextDecoder().decode(input);
		const result = text.replace(
			new RegExp(String(params.pattern), 'g'),
			String(params.replacement ?? '')
		);
		return new TextEncoder().encode(result);
	}
};

export const split: TransformDefinition = {
	id: 'split',
	name: 'Split',
	category: 'text',
	provenance: 'builtin',

	paramSchema: [{ key: 'delimiter', label: 'Delimiter', type: 'string', required: true }],

	async apply(input, params) {
		if (!params.delimiter || typeof params.delimiter !== 'string') {
			throw new TransformError('param_error', 'Missing required parameter: delimiter');
		}
		const text = new TextDecoder().decode(input);
		return new TextEncoder().encode(text.split(String(params.delimiter)).join('\n'));
	}
};

export const reverse: TransformDefinition = {
	id: 'reverse',
	name: 'Reverse',
	category: 'text',
	provenance: 'builtin',

	async apply(input) {
		const output = new Uint8Array(input.length);
		for (let i = 0; i < input.length; i++) {
			output[i] = input[input.length - 1 - i];
		}
		return output;
	}
};

export const textTransforms: TransformDefinition[] = [
	utf8Decode,
	utf16leDecode,
	utf16beDecode,
	regexExtract,
	regexReplace,
	split,
	reverse
];
