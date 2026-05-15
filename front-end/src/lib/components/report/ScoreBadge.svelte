<script lang="ts">
	import type { Classification } from '$lib/api/types';

	interface Props {
		score: number;
		classification?: Classification | string;
	}

	let { score, classification }: Props = $props();
	const clamped = $derived(Math.max(0, Math.min(100, score)));

	const label = $derived(
		classification ? classification.charAt(0).toUpperCase() + classification.slice(1) : 'Unknown'
	);

	const colors = $derived.by(() => {
		switch (classification) {
			case 'malicious':
				return { text: 'text-red-400', dot: 'bg-red-400', bg: 'bg-red-500/10' };
			case 'suspicious':
				return { text: 'text-amber-400', dot: 'bg-amber-400', bg: 'bg-amber-500/10' };
			case 'clean':
				return { text: 'text-emerald-400', dot: 'bg-emerald-400', bg: 'bg-emerald-500/10' };
			default:
				return { text: 'text-zinc-400', dot: 'bg-zinc-500', bg: 'bg-zinc-500/10' };
		}
	});
</script>

<div class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 {colors.bg}">
	<span class="size-1.5 shrink-0 rounded-full {colors.dot}"></span>
	<span class="text-xs font-bold text-[var(--color-text-primary)]">{clamped}</span>
	<span class="text-[11px] font-medium {colors.text}">{label}</span>
</div>
