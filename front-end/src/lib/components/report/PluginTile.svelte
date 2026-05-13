<script lang="ts">
	import ScoreBar from './ScoreBar.svelte';
	import type { PluginReportView } from '$lib/api/types';

	interface Props {
		view: PluginReportView;
		taskId: number;
	}
	let { view, taskId }: Props = $props();

	const report = $derived(view.report);
	const displayName = $derived(
		report?.plugin.display_name ?? report?.plugin.id ?? view.plugin_name
	);
	const iocCount = $derived(report?.indicators?.length ?? 0);
	const ttpCount = $derived(report?.ttps?.length ?? 0);
	const artifactCount = $derived(view.artifacts.length);
</script>

<a
	href={`/submissions/${taskId}/p/${encodeURIComponent(view.plugin_name)}`}
	class="group flex flex-col gap-3 rounded-2xl bg-[var(--color-bg-secondary)] p-5 transition-colors hover:bg-[var(--color-bg-tertiary)]"
>
	<div class="flex items-start justify-between gap-3">
		<div class="min-w-0 space-y-1">
			<div class="truncate text-sm font-semibold text-[var(--color-text-primary)]">
				{displayName}
			</div>
			{#if report?.plugin.version}
				<div class="text-xs text-[var(--color-text-secondary)]">v{report.plugin.version}</div>
			{/if}
		</div>
		{#if report?.verdict?.score !== undefined}
			<div class="shrink-0">
				<ScoreBar score={report.verdict.score} classification={report.verdict.classification} />
			</div>
		{/if}
	</div>

	{#if report?.summary}
		<p class="line-clamp-2 text-xs text-[var(--color-text-secondary)]">{report.summary}</p>
	{/if}

	<div
		class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-[var(--color-text-secondary)]"
	>
		<span>{iocCount} indicator{iocCount === 1 ? '' : 's'}</span>
		<span>·</span>
		<span>{ttpCount} TTP{ttpCount === 1 ? '' : 's'}</span>
		<span>·</span>
		<span>{artifactCount} artifact{artifactCount === 1 ? '' : 's'}</span>
		{#if view.synthesized}
			<span class="ml-auto rounded bg-[var(--color-bg-card)] px-1.5 py-0.5 text-[10px]"
				>synthesized</span
			>
		{/if}
	</div>
</a>
