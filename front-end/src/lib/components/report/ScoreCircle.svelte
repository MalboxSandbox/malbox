<script lang="ts">
	import type { Classification } from '$lib/api/types';

	interface Props {
		score: number;
		classification?: Classification | string;
		size?: number;
		showLabel?: boolean;
	}

	let { score, classification, size = 36, showLabel = true }: Props = $props();
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

	const R = 40;
	const SW = $derived(size < 40 ? 8 : size > 55 ? 6 : 7);
	const fontSize = $derived(`${Math.max(10, Math.min(24, Math.round(size * 0.31)))}px`);
	const glowRadius = $derived(`${Math.round(size * 0.12)}px`);

	const innerPct = $derived(((R - SW / 2) / 50) * 100);
	const outerPct = $derived(((R + SW / 2) / 50) * 100);

	const fillDeg = $derived((clamped / 100) * 360);

	const green = 'oklch(0.75 0.17 145)';
	const yellow = 'oklch(0.80 0.16 85)';
	const red = 'oklch(0.70 0.19 25)';

	let displayDeg = $state(0);

	$effect(() => {
		const target = fillDeg;
		const duration = 800;
		const startTime = performance.now();
		let frame: number;

		function tick(now: number) {
			const progress = Math.min((now - startTime) / duration, 1);
			const eased = 1 - Math.pow(1 - progress, 3);
			displayDeg = target * eased;
			if (progress < 1) {
				frame = requestAnimationFrame(tick);
			}
		}

		frame = requestAnimationFrame(tick);
		return () => cancelAnimationFrame(frame);
	});

	const conicBg = $derived.by(() => {
		const f = displayDeg;
		if (f <= 0) return 'transparent';
		const pct = (f / 360) * 100;
		let stops: string;
		if (pct <= 30) {
			stops = `${green} 0deg, ${green} ${f - 0.5}deg`;
		} else if (pct <= 60) {
			stops = `${green} 0deg, ${yellow} ${f - 0.5}deg`;
		} else {
			stops = `${green} 0deg, ${yellow} ${f * 0.5}deg, ${red} ${f - 0.5}deg`;
		}
		return `conic-gradient(from -90deg, ${stops}, transparent ${f + 0.5}deg)`;
	});

	const ringMask = $derived(
		`radial-gradient(circle farthest-side, transparent ${innerPct - 1}%, black ${innerPct + 1.5}%, black ${outerPct - 1.5}%, transparent ${outerPct + 1}%)`
	);

	const glowColor = $derived.by(() => {
		if (clamped <= 30) return 'oklch(0.75 0.17 145 / 0.3)';
		if (clamped <= 60) return 'oklch(0.80 0.16 85 / 0.3)';
		return 'oklch(0.70 0.19 25 / 0.35)';
	});
</script>

<div class="flex items-center gap-2">
	<div
		class="relative shrink-0"
		style="width: {size}px; height: {size}px; filter: drop-shadow(0 0 {glowRadius} {glowColor});"
	>
		<svg viewBox="0 0 100 100" class="absolute inset-0 size-full" aria-hidden="true">
			<circle cx="50" cy="50" r={R} fill="none" stroke="oklch(0.22 0.005 270)" stroke-width={SW} />
		</svg>

		{#if clamped > 0}
			<div
				class="absolute inset-0 rounded-full"
				style="background: {conicBg}; mask: {ringMask}; -webkit-mask: {ringMask};"
			></div>
		{/if}

		<div class="absolute inset-0 flex items-center justify-center">
			<span
				class="font-bold leading-none text-[var(--color-text-primary)]"
				style="font-size: {fontSize};"
			>
				{clamped}
			</span>
		</div>
	</div>
	{#if showLabel}
		<span class="text-sm font-medium {textColor}">{label}</span>
	{/if}
</div>
