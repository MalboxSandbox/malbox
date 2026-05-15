<script lang="ts">
	import ScoreCircle from './ScoreCircle.svelte';
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

	const bgTint = $derived.by(() => {
		switch (classification) {
			case 'malicious':
				return 'oklch(0.63 0.24 25 / 0.06)';
			case 'suspicious':
				return 'oklch(0.78 0.17 75 / 0.05)';
			case 'clean':
				return 'oklch(0.72 0.19 150 / 0.04)';
			default:
				return 'transparent';
		}
	});
</script>

<div
	class="flex items-center gap-6 rounded-2xl bg-[var(--color-bg-secondary)] px-8 py-6"
	style="background-image: linear-gradient(135deg, {bgTint}, transparent 60%);"
>
	<ScoreCircle {score} {classification} size={64} showLabel={false} />
	<div class="min-w-0">
		<div class="text-lg font-semibold {textColor}">{label}</div>
		<div class="text-sm text-[var(--color-text-secondary)]">{clamped} / 100</div>
	</div>
</div>
