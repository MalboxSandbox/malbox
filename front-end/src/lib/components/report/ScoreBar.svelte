<script lang="ts">
	interface Props {
		score: number | null | undefined;
		max?: number;
		width?: string;
	}

	let { score, max = 10, width = 'w-[90px]' }: Props = $props();

	const clamped = $derived(score != null ? Math.max(0, Math.min(max, score)) : 0);
	const pct = $derived(max > 0 ? (clamped / max) * 100 : 0);

	const fillColor = $derived.by(() => {
		if (score == null) return 'bg-[var(--color-border)]';
		const ratio = clamped / max;
		if (ratio >= 0.7) return 'bg-red-500';
		if (ratio >= 0.4) return 'bg-amber-500';
		return 'bg-emerald-500';
	});
</script>

<div class="flex items-center gap-2.5">
	<div class="{width} h-[5px] overflow-hidden rounded-full bg-[var(--color-border)]">
		<div
			class="{fillColor} h-full rounded-full transition-[width] duration-600 ease-out"
			style="width: {pct}%"
		></div>
	</div>
	<span class="text-xs tabular-nums text-[var(--color-text-secondary)]">
		{score != null ? score : '-'}/{max}
	</span>
</div>
