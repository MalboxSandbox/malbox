import type { ArtifactLink } from '$lib/api/types';

export type PreviewKind = 'image' | 'text' | 'json' | 'code' | 'pdf' | 'none';

const EXT_PREVIEW: Record<string, PreviewKind> = {
	png: 'image',
	jpg: 'image',
	jpeg: 'image',
	gif: 'image',
	bmp: 'image',
	svg: 'image',
	webp: 'image',
	ico: 'image',
	tiff: 'image',
	tif: 'image',
	txt: 'text',
	log: 'text',
	csv: 'text',
	ini: 'text',
	cfg: 'text',
	conf: 'text',
	yaml: 'text',
	yml: 'text',
	toml: 'text',
	xml: 'text',
	html: 'text',
	htm: 'text',
	md: 'text',
	rst: 'text',
	json: 'json',
	py: 'code',
	js: 'code',
	ts: 'code',
	rs: 'code',
	c: 'code',
	cpp: 'code',
	h: 'code',
	hpp: 'code',
	bat: 'code',
	ps1: 'code',
	sh: 'code',
	rb: 'code',
	go: 'code',
	java: 'code',
	cs: 'code',
	asm: 'code',
	yar: 'code',
	yara: 'code',
	lua: 'code',
	pl: 'code',
	php: 'code',
	sql: 'code',
	r: 'code',
	pdf: 'pdf'
};

const EXT_LANG: Record<string, string> = {
	py: 'python',
	js: 'javascript',
	ts: 'typescript',
	rs: 'rust',
	c: 'c',
	cpp: 'c++',
	h: 'c',
	hpp: 'c++',
	sh: 'shell',
	bat: 'batch',
	ps1: 'powershell',
	rb: 'ruby',
	go: 'go',
	java: 'java',
	cs: 'c#',
	asm: 'assembly',
	yar: 'yara',
	yara: 'yara',
	lua: 'lua',
	pl: 'perl',
	php: 'php',
	sql: 'sql',
	r: 'r'
};

function extOf(name: string): string {
	const dot = name.lastIndexOf('.');
	return dot >= 0 ? name.slice(dot + 1).toLowerCase() : '';
}

export function previewKind(name: string, format?: string): PreviewKind {
	if (format === 'json') return 'json';
	return EXT_PREVIEW[extOf(name)] ?? 'none';
}

export function previewLanguage(name: string): string {
	const ext = extOf(name);
	return EXT_LANG[ext] ?? ext;
}

export function resolveArtifactUrl(name: string, artifacts: ArtifactLink[]): string | null {
	return artifacts.find((a) => a.result_name === name)?.url ?? null;
}

export function resolveArtifact(name: string, artifacts: ArtifactLink[]): ArtifactLink | null {
	return artifacts.find((a) => a.result_name === name) ?? null;
}
