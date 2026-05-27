import type { Extension } from '@codemirror/state';
import { StreamLanguage } from '@codemirror/language';

type LanguageLoader = () => Promise<Extension>;

const LANG_LOADERS: Record<string, LanguageLoader> = {
	javascript: () => import('@codemirror/lang-javascript').then((m) => m.javascript()),
	typescript: () =>
		import('@codemirror/lang-javascript').then((m) => m.javascript({ typescript: true })),
	python: () => import('@codemirror/lang-python').then((m) => m.python()),
	json: () => import('@codemirror/lang-json').then((m) => m.json()),
	xml: () => import('@codemirror/lang-xml').then((m) => m.xml()),
	html: () => import('@codemirror/lang-html').then((m) => m.html()),
	css: () => import('@codemirror/lang-css').then((m) => m.css()),
	sql: () => import('@codemirror/lang-sql').then((m) => m.sql()),
	rust: () => import('@codemirror/lang-rust').then((m) => m.rust()),
	c: () => import('@codemirror/lang-cpp').then((m) => m.cpp()),
	cpp: () => import('@codemirror/lang-cpp').then((m) => m.cpp()),
	java: () => import('@codemirror/lang-java').then((m) => m.java()),
	php: () => import('@codemirror/lang-php').then((m) => m.php()),
	go: () => import('@codemirror/lang-go').then((m) => m.go()),
	yaml: () => import('@codemirror/lang-yaml').then((m) => m.yaml()),

	// Legacy StreamLanguage modes
	powershell: () =>
		import('@codemirror/legacy-modes/mode/powershell').then((m) =>
			StreamLanguage.define(m.powerShell)
		),
	lua: () => import('@codemirror/legacy-modes/mode/lua').then((m) => StreamLanguage.define(m.lua)),
	ruby: () =>
		import('@codemirror/legacy-modes/mode/ruby').then((m) => StreamLanguage.define(m.ruby)),
	perl: () =>
		import('@codemirror/legacy-modes/mode/perl').then((m) => StreamLanguage.define(m.perl)),
	shellscript: () =>
		import('@codemirror/legacy-modes/mode/shell').then((m) => StreamLanguage.define(m.shell)),
	csharp: () =>
		import('@codemirror/legacy-modes/mode/clike').then((m) => StreamLanguage.define(m.csharp))
};

const ALIASES: Record<string, string> = {
	js: 'javascript',
	ts: 'typescript',
	py: 'python',
	rs: 'rust',
	'c++': 'cpp',
	'c#': 'csharp',
	shell: 'shellscript',
	sh: 'shellscript',
	bash: 'shellscript',
	bat: 'shellscript',
	batch: 'shellscript',
	ps1: 'powershell',
	rb: 'ruby',
	pl: 'perl',
	yml: 'yaml',
	assembly: 'shellscript'
};

const cache = new Map<string, Extension>();

function resolve(lang: string): string | null {
	const lower = lang.toLowerCase();
	if (lower in LANG_LOADERS) return lower;
	if (lower in ALIASES) return ALIASES[lower];
	return null;
}

export async function loadLanguage(lang: string): Promise<Extension | null> {
	const resolved = resolve(lang);
	if (!resolved) return null;

	const cached = cache.get(resolved);
	if (cached) return cached;

	const ext = await LANG_LOADERS[resolved]();
	cache.set(resolved, ext);
	return ext;
}
