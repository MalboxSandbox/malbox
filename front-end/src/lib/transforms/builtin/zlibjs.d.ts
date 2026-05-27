declare module 'zlibjs/bin/gzip.min.js' {
	const pkg: {
		Zlib: {
			Gzip: new (
				data: Uint8Array,
				options?: Record<string, unknown>
			) => {
				compress(): Uint8Array;
			};
		};
	};
	export default pkg;
}

declare module 'zlibjs/bin/gunzip.min.js' {
	const pkg: {
		Zlib: {
			Gunzip: new (data: Uint8Array) => { decompress(): Uint8Array };
		};
	};
	export default pkg;
}

declare module 'zlibjs/bin/rawdeflate.min.js' {
	const pkg: {
		Zlib: {
			RawDeflate: new (data: Uint8Array) => { compress(): Uint8Array };
		};
	};
	export default pkg;
}

declare module 'zlibjs/bin/rawinflate.min.js' {
	const pkg: {
		Zlib: {
			RawInflate: new (data: Uint8Array) => { decompress(): Uint8Array };
		};
	};
	export default pkg;
}

declare module 'zlibjs/bin/zlib.min.js' {
	const pkg: {
		Zlib: {
			Deflate: new (data: Uint8Array) => { compress(): Uint8Array };
			Inflate: new (data: Uint8Array) => { decompress(): Uint8Array };
		};
	};
	export default pkg;
}
