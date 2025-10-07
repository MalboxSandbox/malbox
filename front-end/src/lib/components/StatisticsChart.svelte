<script lang="ts">
	import type { Statistics } from '$lib/types/automation';

	interface Props {
		stats: Statistics;
	}

	let { stats }: Props = $props();

	const cleanPercentage = $derived(Math.round((stats.clean / stats.totalSubmissions) * 100));
	const suspiciousPercentage = $derived(
		Math.round((stats.suspicious / stats.totalSubmissions) * 100)
	);
	const maliciousPercentage = $derived(
		Math.round((stats.malicious / stats.totalSubmissions) * 100)
	);

	// SVG circle calculations for donut chart
	const radius = 70;
	const strokeWidth = 20;
	const normalizedRadius = radius - strokeWidth / 2;
	const circumference = normalizedRadius * 2 * Math.PI;

	// Calculate stroke dashoffsets for each segment
	const cleanDashArray = $derived((cleanPercentage / 100) * circumference);
	const suspiciousDashArray = $derived((suspiciousPercentage / 100) * circumference);
	const maliciousDashArray = $derived((maliciousPercentage / 100) * circumference);
</script>

<div class="bg-[var(--color-bg-secondary)] rounded-xl p-6">
	<div class="flex items-center justify-between mb-6">
		<h2 class="text-[var(--color-text-primary)] text-lg font-semibold">Statistics</h2>
		<span class="text-[var(--color-text-secondary)] text-sm">
			{stats.totalSubmissions} total submissions
		</span>
	</div>

	<div class="flex items-center gap-12">
		<!-- Donut Chart -->
		<div class="relative">
			<svg width="180" height="180" class="-rotate-90">
				<!-- Background circle -->
				<circle
					cx="90"
					cy="90"
					r={normalizedRadius}
					stroke="var(--color-border)"
					stroke-width={strokeWidth}
					fill="none"
				/>
				<!-- Clean segment (green) -->
				<circle
					cx="90"
					cy="90"
					r={normalizedRadius}
					stroke="var(--color-success)"
					stroke-width={strokeWidth}
					fill="none"
					stroke-dasharray="{cleanDashArray} {circumference}"
					stroke-linecap="round"
					class="transition-all duration-500"
				/>
				<!-- Suspicious segment (yellow/orange) -->
				<circle
					cx="90"
					cy="90"
					r={normalizedRadius}
					stroke="#fb923c"
					stroke-width={strokeWidth}
					fill="none"
					stroke-dasharray="{suspiciousDashArray} {circumference}"
					stroke-dashoffset={-cleanDashArray}
					stroke-linecap="round"
					class="transition-all duration-500"
				/>
				<!-- Malicious segment (red) -->
				<circle
					cx="90"
					cy="90"
					r={normalizedRadius}
					stroke="var(--color-error)"
					stroke-width={strokeWidth}
					fill="none"
					stroke-dasharray="{maliciousDashArray} {circumference}"
					stroke-dashoffset={-(cleanDashArray + suspiciousDashArray)}
					stroke-linecap="round"
					class="transition-all duration-500"
				/>
			</svg>
		</div>

		<!-- Legend -->
		<div class="flex flex-col gap-4">
			<div class="flex items-center gap-3">
				<div class="w-4 h-4 rounded-full bg-[var(--color-success)]"></div>
				<div class="flex items-baseline gap-2">
					<span class="text-[var(--color-text-primary)] text-sm">Clean</span>
					<span class="text-[var(--color-text-secondary)] text-sm">{cleanPercentage}%</span>
				</div>
			</div>
			<div class="flex items-center gap-3">
				<div class="w-4 h-4 rounded-full bg-[#fb923c]"></div>
				<div class="flex items-baseline gap-2">
					<span class="text-[var(--color-text-primary)] text-sm">Suspicious</span>
					<span class="text-[var(--color-text-secondary)] text-sm">{suspiciousPercentage}%</span>
				</div>
			</div>
			<div class="flex items-center gap-3">
				<div class="w-4 h-4 rounded-full bg-[var(--color-error)]"></div>
				<div class="flex items-baseline gap-2">
					<span class="text-[var(--color-text-primary)] text-sm">Malicious</span>
					<span class="text-[var(--color-text-secondary)] text-sm">{maliciousPercentage}%</span>
				</div>
			</div>
		</div>
	</div>
</div>
