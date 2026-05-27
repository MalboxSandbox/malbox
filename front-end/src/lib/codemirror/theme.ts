import { EditorView } from '@codemirror/view';
import { oneDarkHighlightStyle } from '@codemirror/theme-one-dark';
import { syntaxHighlighting } from '@codemirror/language';

const FONT_MONO =
	'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace';
const FONT_SANS = '"Onest", ui-sans-serif, system-ui, sans-serif';

const malboxTheme = EditorView.theme(
	{
		// ── Editor chrome ──────────────────────────────────────
		'&': {
			backgroundColor: 'var(--color-bg-primary)',
			color: 'var(--color-text-primary)',
			fontSize: '12px',
			fontFamily: FONT_MONO
		},
		'.cm-content': {
			caretColor: 'var(--color-accent)',
			padding: '12px 0'
		},
		'.cm-cursor, .cm-dropCursor': {
			borderLeftColor: 'var(--color-accent)'
		},
		'&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection': {
			backgroundColor: 'rgba(81, 108, 249, 0.25)'
		},
		'.cm-activeLine': {
			backgroundColor: 'rgba(255, 255, 255, 0.03)'
		},

		// ── Gutters ───────────────────────────────────────────
		'.cm-gutters': {
			backgroundColor: 'var(--color-bg-primary)',
			color: 'rgba(138, 143, 148, 0.5)',
			border: 'none',
			borderRight: '1px solid rgba(57, 59, 67, 0.3)'
		},
		'.cm-activeLineGutter': {
			backgroundColor: 'transparent',
			color: 'var(--color-text-secondary)'
		},
		'.cm-lineNumbers .cm-gutterElement': {
			padding: '0 12px 0 16px',
			minWidth: '40px'
		},
		'.cm-foldPlaceholder': {
			backgroundColor: 'var(--color-bg-card)',
			color: 'var(--color-text-secondary)',
			border: '1px solid var(--color-border)'
		},

		// ── Scrollbar ─────────────────────────────────────────
		'.cm-scroller': {
			overflow: 'auto',
			scrollbarColor: 'rgba(81, 108, 249, 0.5) rgba(255, 255, 255, 0.04)'
		},
		'.cm-scroller::-webkit-scrollbar': {
			width: '10px',
			height: '10px'
		},
		'.cm-scroller::-webkit-scrollbar-track': {
			background: 'rgba(255, 255, 255, 0.04)',
			borderRadius: '5px'
		},
		'.cm-scroller::-webkit-scrollbar-thumb': {
			backgroundColor: 'rgba(81, 108, 249, 0.5)',
			borderRadius: '5px',
			cursor: 'pointer'
		},
		'.cm-scroller::-webkit-scrollbar-thumb:hover': {
			backgroundColor: 'rgba(81, 108, 249, 0.75)',
			cursor: 'pointer'
		},
		'.cm-scroller::-webkit-scrollbar-thumb:active': {
			backgroundColor: 'rgba(81, 108, 249, 0.9)'
		},
		'.cm-scroller::-webkit-scrollbar-corner': {
			background: 'rgba(255, 255, 255, 0.04)'
		},

		// ── Search: panel container ───────────────────────────
		'.cm-panels': {
			backgroundColor: 'var(--color-bg-tertiary)',
			color: 'var(--color-text-primary)',
			fontFamily: FONT_SANS,
			zIndex: '10'
		},
		'.cm-panels.cm-panels-top': {
			borderBottom: '1px solid var(--color-border)'
		},
		'.cm-panels.cm-panels-bottom': {
			borderTop: '1px solid var(--color-border)'
		},

		// ── Search: layout ────────────────────────────────────
		'.cm-search': {
			display: 'flex !important',
			alignItems: 'center !important',
			gap: '6px !important',
			padding: '8px 12px !important',
			fontFamily: `${FONT_SANS} !important`,
			fontSize: '12px !important',
			lineHeight: '1.4 !important',
			flexWrap: 'nowrap !important',
			overflowX: 'auto !important'
		},
		'.cm-search br': {
			display: 'none !important'
		},
		'.cm-search label.cm-textfield:has(input[name=replace])': {
			display: 'none !important'
		},
		'.cm-search .cm-button[name=replace]': {
			display: 'none !important'
		},
		'.cm-search .cm-button[name=replaceAll]': {
			display: 'none !important'
		},

		// ── Search: text input ────────────────────────────────
		'.cm-search .cm-textfield': {
			backgroundColor: 'var(--color-bg-primary) !important',
			color: 'var(--color-text-primary) !important',
			fontFamily: `${FONT_SANS} !important`,
			fontSize: '12px !important',
			lineHeight: '1.4 !important',
			border: '1px solid var(--color-border) !important',
			borderRadius: '6px !important',
			padding: '5px 10px !important',
			outline: 'none !important',
			width: '200px !important',
			flexShrink: '0'
		},
		'.cm-search .cm-textfield:focus': {
			borderColor: 'var(--color-accent) !important',
			boxShadow: '0 0 0 1px rgba(81, 108, 249, 0.2) !important'
		},

		// ── Search: buttons (next, prev, all) ─────────────────
		'.cm-search .cm-button': {
			backgroundColor: 'transparent !important',
			color: 'var(--color-text-secondary) !important',
			fontFamily: `${FONT_SANS} !important`,
			fontSize: '12px !important',
			lineHeight: '1.4 !important',
			border: 'none !important',
			borderRadius: '6px !important',
			padding: '5px 10px !important',
			cursor: 'pointer !important',
			whiteSpace: 'nowrap !important',
			backgroundImage: 'none !important',
			flexShrink: '0'
		},
		'.cm-search .cm-button:hover': {
			backgroundColor: 'var(--color-bg-card) !important',
			color: 'var(--color-text-primary) !important'
		},
		'.cm-search .cm-button:active': {
			backgroundColor: 'var(--color-tab-active) !important'
		},
		'.cm-search .cm-button[name=select]': {
			opacity: '0.5 !important'
		},

		// ── Search: toggle chips (match case, regexp, by word) ─
		'.cm-search label': {
			display: 'inline-flex !important',
			alignItems: 'center !important',
			gap: '0 !important',
			color: 'var(--color-text-secondary) !important',
			fontFamily: `${FONT_SANS} !important`,
			fontSize: '12px !important',
			lineHeight: '1.4 !important',
			borderRadius: '6px !important',
			padding: '5px 10px !important',
			cursor: 'pointer !important',
			userSelect: 'none !important',
			whiteSpace: 'nowrap !important',
			flexShrink: '0',
			transition: 'color 0.15s, background-color 0.15s !important'
		},
		'.cm-search label:hover': {
			color: 'var(--color-text-primary) !important',
			backgroundColor: 'var(--color-bg-card) !important'
		},
		'.cm-search label:has(input:checked)': {
			backgroundColor: 'rgba(81, 108, 249, 0.15) !important',
			color: 'var(--color-accent) !important'
		},
		'.cm-search input[type=checkbox]': {
			appearance: 'none !important',
			width: '0 !important',
			height: '0 !important',
			margin: '0 !important',
			padding: '0 !important',
			border: 'none !important',
			position: 'absolute !important'
		},

		// ── Search: match highlighting ────────────────────────
		'.cm-searchMatch': {
			backgroundColor: 'rgba(255, 200, 50, 0.3)',
			borderRadius: '2px'
		},
		'.cm-searchMatch.cm-searchMatch-selected': {
			backgroundColor: 'rgba(255, 200, 50, 0.6)'
		},

		// ── Tooltips ──────────────────────────────────────────
		'.cm-tooltip': {
			backgroundColor: 'var(--color-bg-card)',
			color: 'var(--color-text-primary)',
			border: '1px solid var(--color-border)',
			borderRadius: '6px'
		},
		'.cm-tooltip-autocomplete': {
			'& > ul > li[aria-selected]': {
				backgroundColor: 'rgba(81, 108, 249, 0.15)'
			}
		}
	},
	{ dark: true }
);

export const malboxEditorTheme = [malboxTheme, syntaxHighlighting(oneDarkHighlightStyle)];
