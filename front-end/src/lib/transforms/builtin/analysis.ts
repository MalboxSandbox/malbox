import type { TransformDefinition } from '../types';

export const strings: TransformDefinition = {
	id: 'strings',
	name: 'Extract Strings',
	category: 'analysis',
	provenance: 'builtin',

	paramSchema: [
		{
			key: 'min_length',
			label: 'Min Length',
			type: 'number',
			required: false,
			default: 4,
			description: 'Minimum string length'
		}
	],

	async apply(input, params) {
		const minLen = Number(params.min_length ?? 4);
		const found: string[] = [];
		let current = '';

		for (let i = 0; i < input.length; i++) {
			const byte = input[i];
			if (byte >= 0x20 && byte <= 0x7e) {
				current += String.fromCharCode(byte);
			} else {
				if (current.length >= minLen) found.push(current);
				current = '';
			}
		}
		if (current.length >= minLen) found.push(current);

		return new TextEncoder().encode(found.join('\n'));
	}
};

export const entropy: TransformDefinition = {
	id: 'entropy',
	name: 'Shannon Entropy',
	category: 'analysis',
	provenance: 'builtin',

	terminal: true,

	async apply(input) {
		if (input.length === 0) return new TextEncoder().encode('0.000');

		const freq = new Uint32Array(256);
		for (let i = 0; i < input.length; i++) freq[input[i]]++;

		let h = 0;
		for (let i = 0; i < 256; i++) {
			if (freq[i] === 0) continue;
			const p = freq[i] / input.length;
			h -= p * Math.log2(p);
		}

		return new TextEncoder().encode(h.toFixed(4));
	}
};

interface MagicSignature {
	name: string;
	bytes: number[];
	offset?: number;
}

const SIGNATURES: MagicSignature[] = [
	{ name: 'gzip', bytes: [0x1f, 0x8b] },
	{ name: 'PNG', bytes: [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a] },
	{ name: 'JPEG', bytes: [0xff, 0xd8, 0xff] },
	{ name: 'PDF', bytes: [0x25, 0x50, 0x44, 0x46] },
	{ name: 'ZIP', bytes: [0x50, 0x4b, 0x03, 0x04] },
	{ name: 'PE (MZ)', bytes: [0x4d, 0x5a] },
	{ name: 'ELF', bytes: [0x7f, 0x45, 0x4c, 0x46] },
	{ name: 'Mach-O (32)', bytes: [0xfe, 0xed, 0xfa, 0xce] },
	{ name: 'Mach-O (64)', bytes: [0xfe, 0xed, 0xfa, 0xcf] },
	{ name: 'bzip2', bytes: [0x42, 0x5a, 0x68] },
	{ name: 'XZ', bytes: [0xfd, 0x37, 0x7a, 0x58, 0x5a, 0x00] },
	{ name: 'RAR', bytes: [0x52, 0x61, 0x72, 0x21, 0x1a, 0x07] },
	{ name: '7z', bytes: [0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c] },
	{ name: 'SQLite', bytes: [0x53, 0x51, 0x4c, 0x69, 0x74, 0x65] },
	{ name: 'GIF', bytes: [0x47, 0x49, 0x46, 0x38] },
	{ name: 'BMP', bytes: [0x42, 0x4d] },
	{ name: 'PCAP', bytes: [0xd4, 0xc3, 0xb2, 0xa1] },
	{ name: 'PCAP (BE)', bytes: [0xa1, 0xb2, 0xc3, 0xd4] },
	{ name: 'PCAPng', bytes: [0x0a, 0x0d, 0x0d, 0x0a] }
];

export const magicBytes: TransformDefinition = {
	id: 'magic-bytes',
	name: 'Identify File Information',
	category: 'analysis',
	provenance: 'builtin',

	terminal: true,

	async apply(input) {
		const lines: string[] = [];

		lines.push(`Size: ${input.length.toLocaleString()} bytes (${formatSize(input.length)})`);

		let fileType = 'Unknown';
		for (const sig of SIGNATURES) {
			const offset = sig.offset ?? 0;
			if (input.length < offset + sig.bytes.length) continue;
			let match = true;
			for (let i = 0; i < sig.bytes.length; i++) {
				if (input[offset + i] !== sig.bytes[i]) {
					match = false;
					break;
				}
			}
			if (match) {
				fileType = sig.name;
				break;
			}
		}
		lines.push(`File Type: ${fileType}`);

		let isText = true;
		try {
			new TextDecoder('utf-8', { fatal: true }).decode(input);
		} catch {
			isText = false;
		}
		lines.push(`Content: ${isText ? 'UTF-8 Text' : 'Binary'}`);

		if (input.length > 0) {
			const freq = new Uint32Array(256);
			for (let i = 0; i < input.length; i++) freq[input[i]]++;

			let h = 0;
			for (let i = 0; i < 256; i++) {
				if (freq[i] === 0) continue;
				const p = freq[i] / input.length;
				h -= p * Math.log2(p);
			}

			let hint = 'low';
			if (h > 7.5) hint = 'very high - likely compressed or encrypted';
			else if (h > 6) hint = 'high';
			else if (h > 4) hint = 'moderate';
			lines.push(`Entropy: ${h.toFixed(4)} (${hint})`);

			let printable = 0;
			let nullBytes = 0;
			for (let i = 0; i < input.length; i++) {
				if (input[i] >= 0x20 && input[i] <= 0x7e) printable++;
				if (input[i] === 0) nullBytes++;
			}
			lines.push(`Printable: ${((printable / input.length) * 100).toFixed(1)}%`);
			if (nullBytes > 0) {
				lines.push(
					`Null Bytes: ${nullBytes.toLocaleString()} (${((nullBytes / input.length) * 100).toFixed(1)}%)`
				);
			}
		}

		return new TextEncoder().encode(lines.join('\n'));
	}
};

function formatSize(n: number): string {
	if (n < 1024) return `${n} B`;
	if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
	return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

export const frequencyAnalysis: TransformDefinition = {
	id: 'frequency-analysis',
	name: 'Byte Frequency',
	category: 'analysis',
	provenance: 'builtin',

	terminal: true,

	async apply(input) {
		const freq: Record<string, number> = {};
		for (let i = 0; i < input.length; i++) {
			const key = '0x' + input[i].toString(16).padStart(2, '0');
			freq[key] = (freq[key] ?? 0) + 1;
		}
		return new TextEncoder().encode(JSON.stringify(freq));
	}
};

export const analysisTransforms: TransformDefinition[] = [
	strings,
	entropy,
	magicBytes,
	frequencyAnalysis
];
