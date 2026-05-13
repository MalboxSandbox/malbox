<script lang="ts">
	import type { ActivityData, TimeRange } from '$lib/types/automation';

	interface Props {
		data: ActivityData[];
		timeRange: TimeRange;
		onTimeRangeChange: (range: TimeRange) => void;
	}

	let { data, timeRange, onTimeRangeChange }: Props = $props();

	const maxValue = $derived(Math.max(...data.map((d) => d.value)));
	const chartHeight = 200;
	const chartWidth = 500;
	const padding = { top: 20, right: 20, bottom: 30, left: 40 };

	// Calculate points for the area chart
	const points = $derived(
		data.map((d, i) => {
			const x =
				padding.left + (i / (data.length - 1)) * (chartWidth - padding.left - padding.right);
			const y =
				padding.top + (chartHeight - padding.top - padding.bottom) * (1 - d.value / maxValue);
			return { x, y, value: d.value };
		})
	);

	// Create path for area fill
	const areaPath = $derived(() => {
		if (points.length === 0) return '';
		const pathPoints = points.map((p) => `${p.x},${p.y}`).join(' L ');
		const baseY = chartHeight - padding.bottom;
		return `M ${points[0].x},${baseY} L ${pathPoints} L ${points[points.length - 1].x},${baseY} Z`;
	});

	// Create path for line stroke
	const linePath = $derived(() => {
		if (points.length === 0) return '';
		return `M ${points.map((p) => `${p.x},${p.y}`).join(' L ')}`;
	});
</script>

<div class="bg-[var(--color-bg-secondary)] rounded-xl p-6">
	<div class="flex items-center justify-between mb-6">
		<h2 class="text-[var(--color-text-primary)] text-lg font-semibold">Activity</h2>
		<div class="flex gap-2">
			<button
				onclick={() => onTimeRangeChange('day')}
				class="px-4 py-1.5 rounded text-sm transition-colors {timeRange === 'day'
					? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
			>
				Day
			</button>
			<button
				onclick={() => onTimeRangeChange('week')}
				class="px-4 py-1.5 rounded text-sm transition-colors {timeRange === 'week'
					? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
			>
				Week
			</button>
			<button
				onclick={() => onTimeRangeChange('month')}
				class="px-4 py-1.5 rounded text-sm transition-colors {timeRange === 'month'
					? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
			>
				Month
			</button>
		</div>
	</div>

	<div class="relative">
		<svg width={chartWidth} height={chartHeight} class="w-full">
			<!-- Grid lines -->
			{#each Array(5) as _, i (i)}
				<line
					x1={padding.left}
					y1={padding.top + (i * (chartHeight - padding.top - padding.bottom)) / 4}
					x2={chartWidth - padding.right}
					y2={padding.top + (i * (chartHeight - padding.top - padding.bottom)) / 4}
					stroke="var(--color-border)"
					stroke-width="1"
				/>
				<text
					x={padding.left - 10}
					y={padding.top + (i * (chartHeight - padding.top - padding.bottom)) / 4 + 4}
					text-anchor="end"
					class="text-xs fill-[var(--color-text-secondary)]"
				>
					{Math.round(maxValue * (1 - i / 4))}
				</text>
			{/each}

			<!-- Area fill -->
			<path d={areaPath()} fill="url(#gradient)" opacity="0.3" />

			<!-- Line stroke -->
			<path d={linePath()} fill="none" stroke="var(--color-accent)" stroke-width="2" />

			<!-- Gradient definition -->
			<defs>
				<linearGradient id="gradient" x1="0%" y1="0%" x2="0%" y2="100%">
					<stop offset="0%" stop-color="var(--color-accent)" stop-opacity="1" />
					<stop offset="100%" stop-color="var(--color-accent)" stop-opacity="0" />
				</linearGradient>
			</defs>

			<!-- X-axis labels -->
			{#each data as day, i (i)}
				<text
					x={padding.left + (i / (data.length - 1)) * (chartWidth - padding.left - padding.right)}
					y={chartHeight - 10}
					text-anchor="middle"
					class="text-xs fill-[var(--color-text-secondary)]"
				>
					{day.day}
				</text>
			{/each}
		</svg>
	</div>
</div>
