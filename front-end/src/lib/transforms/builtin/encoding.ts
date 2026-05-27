import type { TransformDefinition } from '../types';
import { TransformError } from '../types';

function stripBase64ForApply(raw: string): string {
	return raw
		.replace(/-/g, '+')
		.replace(/_/g, '/')
		.replace(/[^A-Za-z0-9+/=]/g, '');
}

export const base64Decode: TransformDefinition = {
	id: 'base64-decode',
	name: 'Base64 Decode',
	category: 'encoding',
	provenance: 'builtin',

	inverse: 'base64-encode',

	detect(input) {
		if (input.length < 4) return null;
		const text = new TextDecoder().decode(input);
		if (text.length < 4) return null;
		const b64Chars = text.replace(/[^A-Za-z0-9+/\-_=]/g, '').length;
		const ratio = b64Chars / text.length;
		if (ratio < 0.95) return null;
		const normalized = text
			.replace(/-/g, '+')
			.replace(/_/g, '/')
			.replace(/[^A-Za-z0-9+/=]/g, '');
		if (normalized.length < 4) return null;
		if (!/^[A-Za-z0-9+/]+=*$/.test(normalized)) return null;
		if (normalized.length >= 8) return 0.85;
		return 0.6;
	},

	async apply(input) {
		const text = new TextDecoder().decode(input);
		const cleaned = stripBase64ForApply(text);
		if (cleaned.length === 0) {
			throw new TransformError('invalid_input', 'No valid base64 characters found');
		}
		try {
			const binary = atob(cleaned);
			const bytes = new Uint8Array(binary.length);
			for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
			return bytes;
		} catch (e) {
			throw new TransformError(
				'invalid_input',
				`Base64 decode failed: ${e instanceof Error ? e.message : String(e)}`
			);
		}
	}
};

export const base64Encode: TransformDefinition = {
	id: 'base64-encode',
	name: 'Base64 Encode',
	category: 'encoding',
	provenance: 'builtin',

	inverse: 'base64-decode',

	async apply(input) {
		let binary = '';
		for (let i = 0; i < input.length; i++) binary += String.fromCharCode(input[i]);
		return new TextEncoder().encode(btoa(binary));
	}
};

function cleanHex(raw: string): string {
	return raw.replace(/0x/gi, '').replace(/[^0-9a-fA-F]/g, '');
}

export const hexDecode: TransformDefinition = {
	id: 'hex-decode',
	name: 'Hex Decode',
	category: 'encoding',
	provenance: 'builtin',

	inverse: 'hex-encode',

	detect(input) {
		if (input.length < 2) return null;
		const text = new TextDecoder().decode(input);
		const cleaned = cleanHex(text);
		if (cleaned.length < 2 || cleaned.length % 2 !== 0) return null;
		const ratio = cleaned.length / text.length;
		if (ratio < 0.5) return null;
		return cleaned.length >= 8 ? 0.8 * ratio : 0.6 * ratio;
	},

	async apply(input) {
		const text = new TextDecoder().decode(input);
		const cleaned = cleanHex(text);
		if (cleaned.length === 0) {
			throw new TransformError('invalid_input', 'No valid hex characters found');
		}
		const padded = cleaned.length % 2 !== 0 ? '0' + cleaned : cleaned;
		const bytes = new Uint8Array(padded.length / 2);
		for (let i = 0; i < padded.length; i += 2) {
			bytes[i / 2] = parseInt(padded.substring(i, i + 2), 16);
		}
		return bytes;
	}
};

export const hexEncode: TransformDefinition = {
	id: 'hex-encode',
	name: 'Hex Encode',
	category: 'encoding',
	provenance: 'builtin',

	inverse: 'hex-decode',

	async apply(input) {
		const hex = Array.from(input)
			.map((b) => b.toString(16).padStart(2, '0'))
			.join('');
		return new TextEncoder().encode(hex);
	}
};

export const urlDecode: TransformDefinition = {
	id: 'url-decode',
	name: 'URL Decode',
	category: 'encoding',
	provenance: 'builtin',

	inverse: 'url-encode',

	detect(input) {
		const text = new TextDecoder().decode(input);
		const matches = text.match(/%[0-9a-fA-F]{2}/g);
		if (matches && matches.length >= 1) {
			return Math.min(0.7 + matches.length * 0.02, 0.95);
		}
		return null;
	},

	async apply(input) {
		const text = new TextDecoder().decode(input);
		return new TextEncoder().encode(decodeURIComponent(text));
	}
};

export const urlEncode: TransformDefinition = {
	id: 'url-encode',
	name: 'URL Encode',
	category: 'encoding',
	provenance: 'builtin',

	inverse: 'url-decode',

	async apply(input) {
		const text = new TextDecoder().decode(input);
		return new TextEncoder().encode(encodeURIComponent(text));
	}
};

const HTML_ENTITIES: Record<string, string> = {
	'&amp;': '&',
	'&lt;': '<',
	'&gt;': '>',
	'&quot;': '"',
	'&apos;': "'",
	'&nbsp;': ' '
};

export const htmlEntityDecode: TransformDefinition = {
	id: 'html-entity-decode',
	name: 'HTML Entity Decode',
	category: 'encoding',
	provenance: 'builtin',

	detect(input) {
		const text = new TextDecoder().decode(input);
		const matches = text.match(/&(?:#\d+|#x[0-9a-fA-F]+|[a-zA-Z]+);/g);
		if (matches && matches.length >= 1) {
			return Math.min(0.75 + matches.length * 0.02, 0.95);
		}
		return null;
	},

	async apply(input) {
		let text = new TextDecoder().decode(input);
		for (const [entity, char] of Object.entries(HTML_ENTITIES)) {
			text = text.replaceAll(entity, char);
		}
		text = text.replace(/&#(\d+);/g, (_, n) => String.fromCodePoint(parseInt(n, 10)));
		text = text.replace(/&#x([0-9a-fA-F]+);/g, (_, h) => String.fromCodePoint(parseInt(h, 16)));
		return new TextEncoder().encode(text);
	}
};

export const ascii85Decode: TransformDefinition = {
	id: 'ascii85-decode',
	name: 'Ascii85 Decode',
	category: 'encoding',
	provenance: 'builtin',

	detect(input) {
		const text = new TextDecoder().decode(input);
		if (text.startsWith('<~') && text.endsWith('~>')) return 0.95;
		return null;
	},

	async apply(input) {
		let text = new TextDecoder().decode(input);
		if (text.startsWith('<~')) text = text.slice(2);
		if (text.endsWith('~>')) text = text.slice(0, -2);

		const output: number[] = [];
		let i = 0;
		while (i < text.length) {
			if (text[i] === 'z') {
				output.push(0, 0, 0, 0);
				i++;
				continue;
			}
			const group: number[] = [];
			while (group.length < 5 && i < text.length) {
				const c = text.charCodeAt(i);
				if (c >= 33 && c <= 117) group.push(c - 33);
				i++;
			}
			if (group.length === 0) break;
			const padding = 5 - group.length;
			for (let p = 0; p < padding; p++) group.push(84);
			let val = 0;
			for (let j = 0; j < 5; j++) val = val * 85 + group[j];
			const bytes = [(val >>> 24) & 0xff, (val >>> 16) & 0xff, (val >>> 8) & 0xff, val & 0xff];
			output.push(...bytes.slice(0, 4 - padding));
		}
		return new Uint8Array(output);
	}
};

export const quotedPrintableDecode: TransformDefinition = {
	id: 'quoted-printable-decode',
	name: 'Quoted-Printable Decode',
	category: 'encoding',
	provenance: 'builtin',

	detect(input) {
		const text = new TextDecoder().decode(input);
		const matches = text.match(/=[0-9A-Fa-f]{2}/g);
		if (matches && matches.length >= 2) {
			return Math.min(0.7 + matches.length * 0.03, 0.9);
		}
		return null;
	},

	async apply(input) {
		let text = new TextDecoder().decode(input);
		text = text.replace(/=\r?\n/g, '');
		text = text.replace(/=([0-9A-Fa-f]{2})/g, (_, hex) => String.fromCharCode(parseInt(hex, 16)));
		return new TextEncoder().encode(text);
	}
};

const B32_ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';

export const base32Decode: TransformDefinition = {
	id: 'base32-decode',
	name: 'Base32 Decode',
	category: 'encoding',
	provenance: 'builtin',

	detect(input) {
		if (input.length < 8) return null;
		const text = new TextDecoder().decode(input).trim();
		if (text.length < 8 || text.length % 8 !== 0) return null;
		const cleaned = text.replace(/=+$/, '');
		if (!/^[A-Z2-7]+$/i.test(cleaned)) return null;
		return cleaned.length >= 16 ? 0.8 : 0.6;
	},

	async apply(input) {
		const text = new TextDecoder().decode(input).trim().toUpperCase().replace(/=+$/, '');
		let bits = '';
		for (const ch of text) {
			const val = B32_ALPHABET.indexOf(ch);
			if (val === -1) throw new TransformError('invalid_input', `Invalid base32 character: ${ch}`);
			bits += val.toString(2).padStart(5, '0');
		}
		const bytes = new Uint8Array(Math.floor(bits.length / 8));
		for (let i = 0; i < bytes.length; i++) {
			bytes[i] = parseInt(bits.substring(i * 8, i * 8 + 8), 2);
		}
		return bytes;
	}
};

export const base32Encode: TransformDefinition = {
	id: 'base32-encode',
	name: 'Base32 Encode',
	category: 'encoding',
	provenance: 'builtin',

	inverse: 'base32-decode',

	async apply(input) {
		let bits = '';
		for (let i = 0; i < input.length; i++) {
			bits += input[i].toString(2).padStart(8, '0');
		}
		let result = '';
		for (let i = 0; i < bits.length; i += 5) {
			const chunk = bits.substring(i, i + 5).padEnd(5, '0');
			result += B32_ALPHABET[parseInt(chunk, 2)];
		}
		while (result.length % 8 !== 0) result += '=';
		return new TextEncoder().encode(result);
	}
};

export const hexDump: TransformDefinition = {
	id: 'hex-dump',
	name: 'Hex Dump',
	category: 'encoding',
	provenance: 'builtin',
	terminal: true,

	async apply(input) {
		const lines: string[] = [];
		for (let i = 0; i < input.length; i += 16) {
			const slice = input.slice(i, Math.min(i + 16, input.length));
			const offset = i.toString(16).padStart(8, '0');
			const hex = Array.from(slice)
				.map((b) => b.toString(16).padStart(2, '0'))
				.join(' ')
				.padEnd(47, ' ');
			const ascii = Array.from(slice)
				.map((b) => (b >= 0x20 && b <= 0x7e ? String.fromCharCode(b) : '.'))
				.join('');
			lines.push(`${offset}  ${hex}  |${ascii}|`);
		}
		return new TextEncoder().encode(lines.join('\n'));
	}
};

export const encodingTransforms: TransformDefinition[] = [
	base64Decode,
	base64Encode,
	hexDecode,
	hexEncode,
	urlDecode,
	urlEncode,
	htmlEntityDecode,
	ascii85Decode,
	quotedPrintableDecode,
	base32Decode,
	base32Encode,
	hexDump
];
