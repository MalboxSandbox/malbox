<script lang="ts">
	import type { PluginReportView, Confidence } from '$lib/api/types';

	interface Props {
		plugins: PluginReportView[];
	}

	let { plugins }: Props = $props();

	const aiPlugin = $derived.by(() => {
		const synthesized = plugins.find((p) => p.synthesized && p.report?.summary);
		if (synthesized) return synthesized;
		return plugins.find((p) => p.report?.summary && /\bai\b/i.test(p.report.summary)) ?? null;
	});

	const summary = $derived(aiPlugin?.report?.summary ?? null);
	const confidence = $derived(aiPlugin?.report?.verdict?.confidence as Confidence | undefined);

	const confidenceColors = $derived.by(() => {
		switch (confidence) {
			case 'high':
				return 'bg-emerald-500/20 text-emerald-300';
			case 'medium':
				return 'bg-amber-500/20 text-amber-300';
			case 'low':
				return 'bg-red-500/20 text-red-300';
			default:
				return '';
		}
	});
</script>

{#if aiPlugin && summary}
	<div class="space-y-2">
		<div class="flex items-center gap-2">
			<svg
				xmlns="http://www.w3.org/2000/svg"
				viewBox="0 0 20 20"
				fill="currentColor"
				class="size-4 text-[var(--color-accent)]"
			>
				<path
					d="M10 1l2.39 5.34L18 7.24l-4.12 3.82 1.14 5.94L10 14.27 4.98 17l1.14-5.94L2 7.24l5.61-.9L10 1Z"
				/>
			</svg>
			<span class="text-xs font-medium text-[var(--color-accent)]">AI Analysis</span>
			{#if confidence}
				<span class="rounded-md px-1.5 py-0.5 text-[10px] font-medium {confidenceColors}">
					{confidence}
				</span>
			{/if}
		</div>
		<p class="text-[13px] leading-relaxed text-[var(--color-text-secondary)]">{summary}</p>
	</div>
{/if}
