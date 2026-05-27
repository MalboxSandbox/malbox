<script lang="ts">
	import {
		EditorView,
		lineNumbers,
		highlightSpecialChars,
		placeholder as phExt
	} from '@codemirror/view';
	import { EditorState, Compartment } from '@codemirror/state';
	import { search } from '@codemirror/search';
	import { history, defaultKeymap, historyKeymap } from '@codemirror/commands';
	import { keymap } from '@codemirror/view';
	import { untrack } from 'svelte';
	import { malboxEditorTheme } from '$lib/codemirror/theme';
	import { loadLanguage } from '$lib/codemirror/languages';

	interface Props {
		value: string;
		language?: string;
		readonly?: boolean;
		showLineNumbers?: boolean;
		lineWrapping?: boolean;
		maxHeight?: string;
		placeholder?: string;
		onchange?: (value: string) => void;
	}

	let {
		value,
		language = '',
		readonly = false,
		showLineNumbers = true,
		lineWrapping = false,
		maxHeight,
		placeholder = '',
		onchange
	}: Props = $props();

	let containerEl: HTMLDivElement | undefined = $state();
	let view: EditorView | undefined;

	const langCompartment = new Compartment();
	const readonlyCompartment = new Compartment();
	const lineNumbersCompartment = new Compartment();
	const lineWrappingCompartment = new Compartment();
	const placeholderCompartment = new Compartment();
	const maxHeightCompartment = new Compartment();

	export function getView(): EditorView | undefined {
		return view;
	}

	function maxHeightExt(h: string | undefined) {
		if (!h) return [];
		return EditorView.theme({
			'.cm-scroller': { maxHeight: h, overflowY: 'auto' }
		});
	}

	function createView(parent: HTMLElement) {
		const updateListener = EditorView.updateListener.of((update) => {
			if (update.docChanged && onchange) {
				onchange(update.state.doc.toString());
			}
		});

		const state = EditorState.create({
			doc: value,
			extensions: [
				malboxEditorTheme,
				highlightSpecialChars(),
				lineNumbersCompartment.of(showLineNumbers ? lineNumbers() : []),
				lineWrappingCompartment.of(lineWrapping ? EditorView.lineWrapping : []),
				readonlyCompartment.of(EditorState.readOnly.of(readonly)),
				langCompartment.of([]),
				placeholderCompartment.of(placeholder ? phExt(placeholder) : []),
				maxHeightCompartment.of(maxHeightExt(maxHeight)),
				search({ top: true }),
				history(),
				keymap.of([...defaultKeymap, ...historyKeymap]),
				updateListener
			]
		});

		view = new EditorView({ state, parent });
	}

	$effect(() => {
		if (!containerEl) return;
		untrack(() => createView(containerEl!));
		return () => {
			view?.destroy();
			view = undefined;
		};
	});

	$effect(() => {
		if (!view) return;
		const current = view.state.doc.toString();
		if (value !== current) {
			view.dispatch({
				changes: { from: 0, to: current.length, insert: value }
			});
		}
	});

	$effect(() => {
		if (!view) return;
		view.dispatch({
			effects: readonlyCompartment.reconfigure(EditorState.readOnly.of(readonly))
		});
	});

	$effect(() => {
		if (!view) return;
		view.dispatch({
			effects: lineNumbersCompartment.reconfigure(showLineNumbers ? lineNumbers() : [])
		});
	});

	$effect(() => {
		if (!view) return;
		view.dispatch({
			effects: lineWrappingCompartment.reconfigure(lineWrapping ? EditorView.lineWrapping : [])
		});
	});

	$effect(() => {
		if (!view) return;
		loadLanguage(language).then((ext) => {
			if (view) {
				view.dispatch({ effects: langCompartment.reconfigure(ext ?? []) });
			}
		});
	});

	$effect(() => {
		if (!view) return;
		view.dispatch({
			effects: placeholderCompartment.reconfigure(placeholder ? phExt(placeholder) : [])
		});
	});

	$effect(() => {
		if (!view) return;
		view.dispatch({
			effects: maxHeightCompartment.reconfigure(maxHeightExt(maxHeight))
		});
	});
</script>

<div bind:this={containerEl} class="cm-host"></div>

<style>
	.cm-host {
		height: 100%;
		min-height: 0;
	}
	.cm-host :global(.cm-editor) {
		height: 100%;
	}
	.cm-host :global(.cm-scroller) {
		overflow: auto;
	}

	/* Close button - override outside CM theme system for full control */
	.cm-host :global(.cm-search button[name='close']) {
		margin-left: auto;
		width: 28px;
		height: 28px;
		display: flex !important;
		align-items: center;
		justify-content: center;
		padding: 0 !important;
		font-size: 0 !important;
		border-radius: 6px;
		color: var(--color-text-secondary);
		background: transparent;
		border: none;
		cursor: pointer;
		flex-shrink: 0;
		position: relative;
	}
	.cm-host :global(.cm-search button[name='close'])::after {
		content: '';
		width: 14px;
		height: 14px;
		background-color: var(--color-text-secondary);
		mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2.5' stroke-linecap='round'%3E%3Cpath d='M18 6L6 18M6 6l12 12'/%3E%3C/svg%3E");
		mask-size: contain;
		mask-repeat: no-repeat;
	}
	.cm-host :global(.cm-search button[name='close']:hover) {
		background: var(--color-bg-card);
	}
	.cm-host :global(.cm-search button[name='close']:hover)::after {
		background-color: var(--color-text-primary);
	}

	/* Search input - use project font for placeholder */
	.cm-host :global(.cm-search .cm-textfield input) {
		font-family: 'Onest', ui-sans-serif, system-ui, sans-serif !important;
		font-size: 12px !important;
	}
</style>
