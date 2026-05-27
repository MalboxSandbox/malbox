import type { TransformDefinition } from '../types';
import { TransformError } from '../types';

// zlibjs uses a UMD/IIFE pattern that Rollup can't statically analyze for
// named exports, so we lazy-load via dynamic import to avoid SSR build failures.
interface ZlibModules {
	Gzip: new (data: Uint8Array) => { compress(): Uint8Array };
	Gunzip: new (data: Uint8Array) => { decompress(): Uint8Array };
	RawDeflate: new (data: Uint8Array) => { compress(): Uint8Array };
	RawInflate: new (data: Uint8Array) => { decompress(): Uint8Array };
	Inflate: new (data: Uint8Array) => { decompress(): Uint8Array };
	Deflate: new (data: Uint8Array) => { compress(): Uint8Array };
}

let _cached: ZlibModules | null = null;

async function zlib(): Promise<ZlibModules> {
	if (_cached) return _cached;
	const [gzip, gunzip, rawdeflate, rawinflate, zmod] = await Promise.all([
		import('zlibjs/bin/gzip.min.js'),
		import('zlibjs/bin/gunzip.min.js'),
		import('zlibjs/bin/rawdeflate.min.js'),
		import('zlibjs/bin/rawinflate.min.js'),
		import('zlibjs/bin/zlib.min.js')
	]);
	const unwrap = (m: Record<string, unknown>) => (m.Zlib ?? (m.default as Record<string, unknown>)?.Zlib) as Record<string, unknown>;
	const g = unwrap(gzip);
	const u = unwrap(gunzip);
	const rd = unwrap(rawdeflate);
	const ri = unwrap(rawinflate);
	const z = unwrap(zmod);
	_cached = {
		Gzip: g.Gzip as ZlibModules['Gzip'],
		Gunzip: u.Gunzip as ZlibModules['Gunzip'],
		RawDeflate: rd.RawDeflate as ZlibModules['RawDeflate'],
		RawInflate: ri.RawInflate as ZlibModules['RawInflate'],
		Inflate: z.Inflate as ZlibModules['Inflate'],
		Deflate: z.Deflate as ZlibModules['Deflate']
	};
	return _cached;
}

function gzipDeflateOffset(data: Uint8Array): number {
	if (data.length < 10 || data[0] !== 0x1f || data[1] !== 0x8b) return -1;
	const flags = data[3];
	let offset = 10;
	if (flags & 0x04) {
		if (offset + 2 > data.length) return -1;
		offset += 2 + (data[offset] | (data[offset + 1] << 8));
	}
	if (flags & 0x08) {
		while (offset < data.length && data[offset] !== 0) offset++;
		offset++;
	}
	if (flags & 0x10) {
		while (offset < data.length && data[offset] !== 0) offset++;
		offset++;
	}
	if (flags & 0x02) offset += 2;
	return offset < data.length ? offset : -1;
}

export const gunzip: TransformDefinition = {
	id: 'gunzip',
	name: 'Gunzip',
	category: 'compression',
	provenance: 'builtin',

	inverse: 'gzip',

	detect(input) {
		if (input.length >= 2 && input[0] === 0x1f && input[1] === 0x8b) return 0.95;
		return null;
	},

	async apply(input) {
		const z = await zlib();
		try {
			return new z.Gunzip(input).decompress();
		} catch (e) {
			const offset = gzipDeflateOffset(input);
			if (offset > 0) {
				try {
					return new z.RawInflate(input.subarray(offset)).decompress();
				} catch {
					// fall through to original error
				}
			}
			throw new TransformError(
				'invalid_input',
				`Gunzip failed: ${e instanceof Error ? e.message : String(e)}`
			);
		}
	}
};

export const gzip: TransformDefinition = {
	id: 'gzip',
	name: 'Gzip',
	category: 'compression',
	provenance: 'builtin',

	inverse: 'gunzip',

	async apply(input) {
		const z = await zlib();
		return new z.Gzip(input).compress();
	}
};

export const inflate: TransformDefinition = {
	id: 'inflate',
	name: 'Inflate (raw deflate)',
	category: 'compression',
	provenance: 'builtin',

	inverse: 'deflate',

	detect(input) {
		if (input.length < 2) return null;
		const cmf = input[0];
		const flg = input[1];
		if ((cmf & 0x0f) === 8 && (cmf * 256 + flg) % 31 === 0) return 0.75;
		return null;
	},

	async apply(input) {
		const z = await zlib();
		try {
			return new z.RawInflate(input).decompress();
		} catch (e) {
			throw new TransformError(
				'invalid_input',
				`Inflate failed: ${e instanceof Error ? e.message : String(e)}`
			);
		}
	}
};

export const deflate: TransformDefinition = {
	id: 'deflate',
	name: 'Deflate',
	category: 'compression',
	provenance: 'builtin',

	inverse: 'inflate',

	async apply(input) {
		const z = await zlib();
		return new z.RawDeflate(input).compress();
	}
};

export const bzip2Decompress: TransformDefinition = {
	id: 'bzip2-decompress',
	name: 'Bzip2 Decompress',
	category: 'compression',
	provenance: 'builtin',

	detect(input) {
		if (input.length >= 3 && input[0] === 0x42 && input[1] === 0x5a && input[2] === 0x68)
			return 0.95;
		return null;
	},

	async apply() {
		throw new TransformError(
			'runtime_error',
			'Bzip2 requires a WASM transform module. Install one via Settings > Transforms.'
		);
	}
};

export const xzDecompress: TransformDefinition = {
	id: 'xz-decompress',
	name: 'XZ Decompress',
	category: 'compression',
	provenance: 'builtin',

	detect(input) {
		if (
			input.length >= 6 &&
			input[0] === 0xfd &&
			input[1] === 0x37 &&
			input[2] === 0x7a &&
			input[3] === 0x58 &&
			input[4] === 0x5a &&
			input[5] === 0x00
		)
			return 0.95;
		return null;
	},

	async apply() {
		throw new TransformError(
			'runtime_error',
			'XZ requires a WASM transform module. Install one via Settings > Transforms.'
		);
	}
};

export const zlibDecompress: TransformDefinition = {
	id: 'zlib-decompress',
	name: 'Zlib Decompress',
	category: 'compression',
	provenance: 'builtin',

	inverse: 'zlib-compress',

	detect(input) {
		if (input.length < 6) return null;
		const cmf = input[0];
		const flg = input[1];
		if ((cmf & 0x0f) !== 8) return null;
		if ((cmf * 256 + flg) % 31 !== 0) return null;
		if (input[0] === 0x1f && input[1] === 0x8b) return null;
		return 0.8;
	},

	async apply(input) {
		const z = await zlib();
		try {
			return new z.Inflate(input).decompress();
		} catch (e) {
			throw new TransformError(
				'invalid_input',
				`Zlib decompress failed: ${e instanceof Error ? e.message : String(e)}`
			);
		}
	}
};

export const zlibCompress: TransformDefinition = {
	id: 'zlib-compress',
	name: 'Zlib Compress',
	category: 'compression',
	provenance: 'builtin',

	inverse: 'zlib-decompress',

	async apply(input) {
		const z = await zlib();
		return new z.Deflate(input).compress();
	}
};

export const compressionTransforms: TransformDefinition[] = [
	gunzip,
	gzip,
	inflate,
	deflate,
	zlibDecompress,
	zlibCompress,
	bzip2Decompress,
	xzDecompress
];
