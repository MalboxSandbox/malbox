import type { TransformDefinition } from '../types';
import { TransformError } from '../types';

function parseHexKey(keyStr: string): Uint8Array {
	let hex = keyStr.startsWith('0x') || keyStr.startsWith('0X') ? keyStr.slice(2) : keyStr;
	if (hex.length % 2 !== 0) hex = '0' + hex;
	const bytes = new Uint8Array(hex.length / 2);
	for (let i = 0; i < hex.length; i += 2) {
		bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
	}
	return bytes;
}

export const xor: TransformDefinition = {
	id: 'xor',
	name: 'XOR',
	category: 'crypto',
	provenance: 'builtin',

	paramSchema: [
		{
			key: 'key',
			label: 'Key (hex)',
			type: 'string',
			required: true,
			description: 'Hex key, e.g. 0xFF or 0xDEAD'
		}
	],

	async apply(input, params) {
		if (!params.key || typeof params.key !== 'string') {
			throw new TransformError('param_error', 'Missing required parameter: key');
		}
		const key = parseHexKey(String(params.key));
		const output = new Uint8Array(input.length);
		for (let i = 0; i < input.length; i++) {
			output[i] = input[i] ^ key[i % key.length];
		}
		return output;
	}
};

export const rc4: TransformDefinition = {
	id: 'rc4',
	name: 'RC4',
	category: 'crypto',
	provenance: 'builtin',

	paramSchema: [{ key: 'key', label: 'Key', type: 'string', required: true }],

	async apply(input, params) {
		if (!params.key || typeof params.key !== 'string') {
			throw new TransformError('param_error', 'Missing required parameter: key');
		}
		const keyBytes = new TextEncoder().encode(String(params.key));
		const s = new Uint8Array(256);
		for (let i = 0; i < 256; i++) s[i] = i;

		let j = 0;
		for (let i = 0; i < 256; i++) {
			j = (j + s[i] + keyBytes[i % keyBytes.length]) & 0xff;
			[s[i], s[j]] = [s[j], s[i]];
		}

		const output = new Uint8Array(input.length);
		let ii = 0;
		j = 0;
		for (let k = 0; k < input.length; k++) {
			ii = (ii + 1) & 0xff;
			j = (j + s[ii]) & 0xff;
			[s[ii], s[j]] = [s[j], s[ii]];
			output[k] = input[k] ^ s[(s[ii] + s[j]) & 0xff];
		}
		return output;
	}
};

function rotateChar(code: number, shift: number): number {
	if (code >= 65 && code <= 90) return ((((code - 65 + shift) % 26) + 26) % 26) + 65;
	if (code >= 97 && code <= 122) return ((((code - 97 + shift) % 26) + 26) % 26) + 97;
	return code;
}

export const rot13: TransformDefinition = {
	id: 'rot13',
	name: 'ROT13',
	category: 'crypto',
	provenance: 'builtin',

	inverse: 'rot13',

	async apply(input) {
		const output = new Uint8Array(input.length);
		for (let i = 0; i < input.length; i++) {
			output[i] = rotateChar(input[i], 13);
		}
		return output;
	}
};

export const rot: TransformDefinition = {
	id: 'rot',
	name: 'ROT-N',
	category: 'crypto',
	provenance: 'builtin',

	paramSchema: [
		{
			key: 'shift',
			label: 'Shift',
			type: 'number',
			required: true,
			description: 'Number of positions to shift (1-25)'
		}
	],

	async apply(input, params) {
		const shift = Number(params.shift ?? 13);
		const output = new Uint8Array(input.length);
		for (let i = 0; i < input.length; i++) {
			output[i] = rotateChar(input[i], shift);
		}
		return output;
	}
};

export const slice: TransformDefinition = {
	id: 'slice',
	name: 'Slice',
	category: 'crypto',
	provenance: 'builtin',

	paramSchema: [
		{
			key: 'offset',
			label: 'Offset',
			type: 'number',
			required: true,
			description: 'Start byte offset'
		},
		{
			key: 'length',
			label: 'Length',
			type: 'number',
			required: false,
			description: 'Number of bytes (omit for rest of input)'
		}
	],

	async apply(input, params) {
		const offset = Number(params.offset ?? 0);
		const length = params.length !== undefined ? Number(params.length) : undefined;
		if (offset < 0 || offset > input.length) {
			throw new TransformError('param_error', `Offset ${offset} out of range (0-${input.length})`);
		}
		return length !== undefined ? input.slice(offset, offset + length) : input.slice(offset);
	}
};

export const bitwiseNot: TransformDefinition = {
	id: 'not',
	name: 'Bitwise NOT',
	category: 'crypto',
	provenance: 'builtin',

	inverse: 'not',

	async apply(input) {
		const output = new Uint8Array(input.length);
		for (let i = 0; i < input.length; i++) {
			output[i] = ~input[i] & 0xff;
		}
		return output;
	}
};

export const aesDecrypt: TransformDefinition = {
	id: 'aes-decrypt',
	name: 'AES Decrypt',
	category: 'crypto',
	provenance: 'builtin',

	paramSchema: [
		{ key: 'key', label: 'Key (hex)', type: 'string', required: true },
		{ key: 'iv', label: 'IV (hex)', type: 'string', required: true },
		{ key: 'mode', label: 'Mode', type: 'select', required: true, options: ['CBC', 'CTR', 'GCM'] }
	],

	async apply(input, params) {
		if (!params.key || !params.iv || !params.mode) {
			throw new TransformError('param_error', 'Missing required parameters: key, iv, mode');
		}
		const keyBytes = parseHexKey(String(params.key));
		const ivBytes = parseHexKey(String(params.iv));
		const modeMap: Record<string, string> = { CBC: 'AES-CBC', CTR: 'AES-CTR', GCM: 'AES-GCM' };
		const alg = modeMap[String(params.mode)];
		if (!alg) throw new TransformError('param_error', `Unknown AES mode: ${params.mode}`);

		try {
			const cryptoKey = await crypto.subtle.importKey(
				'raw',
				keyBytes.buffer as ArrayBuffer,
				alg,
				false,
				['decrypt']
			);
			const algParam =
				alg === 'AES-CTR'
					? { name: alg, counter: ivBytes.buffer as ArrayBuffer, length: 128 }
					: { name: alg, iv: ivBytes.buffer as ArrayBuffer };
			const result = await crypto.subtle.decrypt(algParam, cryptoKey, input.buffer as ArrayBuffer);
			return new Uint8Array(result);
		} catch (e) {
			throw new TransformError(
				'invalid_input',
				`AES decryption failed: ${e instanceof Error ? e.message : String(e)}`
			);
		}
	}
};

function shannonEntropy(data: Uint8Array): number {
	if (data.length === 0) return 0;
	const freq = new Uint32Array(256);
	for (let i = 0; i < data.length; i++) freq[data[i]]++;
	let h = 0;
	for (let i = 0; i < 256; i++) {
		if (freq[i] === 0) continue;
		const p = freq[i] / data.length;
		h -= p * Math.log2(p);
	}
	return h;
}

export const xorBruteForce: TransformDefinition = {
	id: 'xor-bruteforce',
	name: 'XOR Brute-force',
	category: 'crypto',
	provenance: 'builtin',
	terminal: true,

	paramSchema: [
		{
			key: 'top_n',
			label: 'Top N results',
			type: 'number',
			required: false,
			default: 10,
			description: 'Number of results to show (sorted by lowest entropy)'
		}
	],

	async apply(input, params) {
		const topN = Number(params.top_n ?? 10);
		const sampleLen = Math.min(input.length, 1024);
		const sample = input.slice(0, sampleLen);

		const results: { key: number; entropy: number; printableRatio: number; preview: string }[] = [];

		for (let key = 1; key < 256; key++) {
			const decoded = new Uint8Array(sampleLen);
			let printable = 0;
			for (let i = 0; i < sampleLen; i++) {
				decoded[i] = sample[i] ^ key;
				if (decoded[i] >= 0x20 && decoded[i] <= 0x7e) printable++;
			}
			results.push({
				key,
				entropy: shannonEntropy(decoded),
				printableRatio: printable / sampleLen,
				preview: Array.from(decoded.slice(0, 64))
					.map((b) => (b >= 0x20 && b <= 0x7e ? String.fromCharCode(b) : '.'))
					.join('')
			});
		}

		results.sort((a, b) => a.entropy - b.entropy);

		const lines = results.slice(0, topN).map((r) => {
			const keyHex = '0x' + r.key.toString(16).padStart(2, '0');
			return `Key: ${keyHex}  Entropy: ${r.entropy.toFixed(3)}  Printable: ${(r.printableRatio * 100).toFixed(1)}%  Preview: ${r.preview}`;
		});
		return new TextEncoder().encode(lines.join('\n'));
	}
};

export const xorKeySearch: TransformDefinition = {
	id: 'xor-key-search',
	name: 'XOR Known-plaintext',
	category: 'crypto',
	provenance: 'builtin',

	paramSchema: [
		{
			key: 'known',
			label: 'Known plaintext',
			type: 'string',
			required: true,
			description: 'Expected plaintext (e.g. "MZ", "This program")'
		}
	],

	async apply(input, params) {
		if (!params.known || typeof params.known !== 'string') {
			throw new TransformError('param_error', 'Missing required parameter: known');
		}
		const known = new TextEncoder().encode(String(params.known));
		if (known.length === 0 || known.length > input.length) {
			throw new TransformError('param_error', 'Known plaintext too long or empty');
		}

		const key = new Uint8Array(known.length);
		for (let i = 0; i < known.length; i++) {
			key[i] = input[i] ^ known[i];
		}

		const output = new Uint8Array(input.length);
		for (let i = 0; i < input.length; i++) {
			output[i] = input[i] ^ key[i % key.length];
		}
		return output;
	}
};

export const xorMultiByteDetect: TransformDefinition = {
	id: 'xor-multikey-detect',
	name: 'XOR Multi-byte Key Detect',
	category: 'crypto',
	provenance: 'builtin',
	terminal: true,

	paramSchema: [
		{
			key: 'max_key_len',
			label: 'Max key length',
			type: 'number',
			required: false,
			default: 16,
			description: 'Maximum key length to test (1-32)'
		}
	],

	async apply(input, params) {
		const maxKeyLen = Math.min(Number(params.max_key_len ?? 16), 32);
		if (input.length < 8) {
			throw new TransformError('invalid_input', 'Input too short for key detection');
		}

		const results: { keyLen: number; key: string; entropy: number; preview: string }[] = [];

		for (let keyLen = 1; keyLen <= maxKeyLen && keyLen <= input.length / 2; keyLen++) {
			const key = new Uint8Array(keyLen);
			for (let pos = 0; pos < keyLen; pos++) {
				const freq = new Uint32Array(256);
				for (let i = pos; i < input.length; i += keyLen) {
					freq[input[i]]++;
				}
				let bestByte = 0;
				let bestScore = -1;
				for (let b = 0; b < 256; b++) {
					const candidate = b;
					let score = 0;
					for (let i = pos; i < input.length; i += keyLen) {
						const decoded = input[i] ^ candidate;
						if (decoded >= 0x20 && decoded <= 0x7e) score++;
						if (decoded === 0x20 || (decoded >= 0x61 && decoded <= 0x7a)) score++;
					}
					if (score > bestScore) {
						bestScore = score;
						bestByte = candidate;
					}
				}
				key[pos] = bestByte;
			}

			const decoded = new Uint8Array(Math.min(input.length, 128));
			for (let i = 0; i < decoded.length; i++) {
				decoded[i] = input[i] ^ key[i % keyLen];
			}

			results.push({
				keyLen,
				key: Array.from(key)
					.map((b) => b.toString(16).padStart(2, '0'))
					.join(''),
				entropy: shannonEntropy(decoded),
				preview: Array.from(decoded.slice(0, 64))
					.map((b) => (b >= 0x20 && b <= 0x7e ? String.fromCharCode(b) : '.'))
					.join('')
			});
		}

		results.sort((a, b) => a.entropy - b.entropy);

		const lines = results.slice(0, 5).map((r) => {
			return `KeyLen: ${r.keyLen}  Key: 0x${r.key}  Entropy: ${r.entropy.toFixed(3)}  Preview: ${r.preview}`;
		});
		return new TextEncoder().encode(lines.join('\n'));
	}
};

export const deobfuscateStrings: TransformDefinition = {
	id: 'deobfuscate-strings',
	name: 'Deobfuscate Strings',
	category: 'crypto',
	provenance: 'builtin',

	paramSchema: [
		{
			key: 'min_length',
			label: 'Min string length',
			type: 'number',
			required: false,
			default: 6,
			description: 'Minimum decoded string length'
		}
	],

	async apply(input, params) {
		const minLen = Number(params.min_length ?? 6);
		const found: { offset: number; key: number; text: string }[] = [];

		for (let key = 1; key < 256; key++) {
			let current = '';
			let startOffset = 0;
			for (let i = 0; i < input.length; i++) {
				const decoded = input[i] ^ key;
				if (decoded >= 0x20 && decoded <= 0x7e) {
					if (current.length === 0) startOffset = i;
					current += String.fromCharCode(decoded);
				} else {
					if (current.length >= minLen) {
						found.push({ offset: startOffset, key, text: current });
					}
					current = '';
				}
			}
			if (current.length >= minLen) {
				found.push({ offset: startOffset, key, text: current });
			}
		}

		found.sort((a, b) => b.text.length - a.text.length);

		const seen = new Set<string>();
		const unique = found.filter((f) => {
			if (seen.has(f.text)) return false;
			seen.add(f.text);
			return true;
		});

		const lines = unique.slice(0, 100).map((f) => {
			const keyHex = '0x' + f.key.toString(16).padStart(2, '0');
			const offsetHex = '0x' + f.offset.toString(16).padStart(4, '0');
			return `[${offsetHex}] key=${keyHex} "${f.text}"`;
		});
		return new TextEncoder().encode(lines.join('\n') || 'No deobfuscated strings found');
	}
};

export const cryptoTransforms: TransformDefinition[] = [
	xor,
	rc4,
	rot13,
	rot,
	slice,
	bitwiseNot,
	aesDecrypt,
	xorBruteForce,
	xorKeySearch,
	xorMultiByteDetect,
	deobfuscateStrings
];
