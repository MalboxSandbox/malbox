<script lang="ts">
	import type { Classification } from '$lib/api/types';

	interface Props {
		score: number;
		classification?: Classification | string;
	}

	let { score, classification }: Props = $props();
	const clamped = $derived(Math.max(0, Math.min(100, score)));

	const textColor = $derived.by(() => {
		switch (classification) {
			case 'malicious':
				return 'text-red-400';
			case 'suspicious':
				return 'text-amber-400';
			case 'clean':
				return 'text-emerald-400';
			default:
				return 'text-[var(--color-text-secondary)]';
		}
	});

	const label = $derived(
		classification ? classification.charAt(0).toUpperCase() + classification.slice(1) : 'Unknown'
	);

	const gray = 'oklch(0.35 0 0)';

	const barBackground = $derived.by(() => {
		const pct = clamped;
		if (pct === 0) return gray;

		const colorStops: string[] = [];
		if (pct <= 30) {
			colorStops.push('oklch(0.75 0.17 145)');
			colorStops.push('oklch(0.75 0.17 145)');
		} else if (pct <= 60) {
			colorStops.push('oklch(0.75 0.17 145)');
			colorStops.push('oklch(0.80 0.16 85)');
		} else {
			colorStops.push('oklch(0.75 0.17 145)');
			colorStops.push('oklch(0.80 0.16 85)');
			colorStops.push('oklch(0.70 0.19 25)');
		}

		const stops = colorStops.map((c, i) => {
			const pos = (i / (colorStops.length - 1)) * pct;
			return `${c} ${pos}%`;
		});
		stops.push(`${gray} ${pct}%`);
		stops.push(`${gray} 100%`);

		return `linear-gradient(to right, ${stops.join(', ')})`;
	});
</script>

<div class="flex items-center gap-3">
	<span class="text-sm font-medium {textColor}">{label}</span>
	<div
		class="h-1.5 w-24 rounded-full transition-all duration-500 ease-out"
		style="background: {barBackground};"
	></div>
	<span class="text-sm text-[var(--color-text-secondary)]">{clamped}/100</span>
</div>
