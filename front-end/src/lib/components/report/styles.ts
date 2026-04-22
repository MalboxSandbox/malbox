import type { CalloutLevel, Classification } from '$lib/api/types';

/**
 * Tailwind class set for a verdict classification — use for pills, borders,
 * and tinted card backgrounds. Returns an object so callers can pick the
 * slot they need.
 */
export function classificationClasses(c: Classification | string | undefined) {
	switch (c) {
		case 'malicious':
			return {
				pill: 'bg-red-500/20 text-red-300 border border-red-500/40',
				border: 'border-red-500/40',
				tint: 'bg-red-500/10',
				dot: 'bg-red-400'
			};
		case 'suspicious':
			return {
				pill: 'bg-amber-500/20 text-amber-300 border border-amber-500/40',
				border: 'border-amber-500/40',
				tint: 'bg-amber-500/10',
				dot: 'bg-amber-400'
			};
		case 'clean':
			return {
				pill: 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/40',
				border: 'border-emerald-500/40',
				tint: 'bg-emerald-500/10',
				dot: 'bg-emerald-400'
			};
		default:
			return {
				pill: 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] border border-[var(--color-border)]',
				border: 'border-[var(--color-border)]',
				tint: '',
				dot: 'bg-[var(--color-text-secondary)]'
			};
	}
}

export function calloutClasses(level: CalloutLevel) {
	switch (level) {
		case 'error':
			return 'border-red-500/40 bg-red-500/10 text-red-200';
		case 'warn':
			return 'border-amber-500/40 bg-amber-500/10 text-amber-200';
		case 'success':
			return 'border-emerald-500/40 bg-emerald-500/10 text-emerald-200';
		case 'info':
		default:
			return 'border-sky-500/40 bg-sky-500/10 text-sky-200';
	}
}

export function severityDotClass(severity: string | undefined): string {
	switch ((severity ?? '').toLowerCase()) {
		case 'critical':
			return 'bg-red-600';
		case 'high':
			return 'bg-red-400';
		case 'medium':
			return 'bg-amber-400';
		case 'low':
			return 'bg-slate-400';
		case 'info':
		default:
			return 'bg-sky-400';
	}
}
