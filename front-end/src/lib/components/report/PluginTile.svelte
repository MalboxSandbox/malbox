<script lang="ts">
	import ScoreBadge from './ScoreBadge.svelte';
	import type { PluginReportView } from '$lib/api/types';

	interface Props {
		view: PluginReportView;
		taskId: number;
		sha256?: string;
	}
	let { view, taskId, sha256 }: Props = $props();

	const report = $derived(view.report);
	const displayName = $derived(
		report?.plugin.display_name ?? report?.plugin.id ?? view.plugin_name
	);
	const iocCount = $derived(report?.indicators?.length ?? 0);
	const ttpCount = $derived(report?.ttps?.length ?? 0);
	const artifactCount = $derived(view.artifacts.length);
</script>

{#if view.failed}
	<div class="group flex flex-col gap-3 rounded-2xl bg-[var(--color-bg-secondary)] p-5 opacity-70">
		<div class="flex items-start justify-between gap-3">
			<div class="min-w-0">
				<span class="truncate text-sm font-semibold text-[var(--color-text-primary)]">
					{displayName}
				</span>
			</div>
			<span class="shrink-0 rounded bg-red-500/20 px-2 py-0.5 text-xs font-medium text-red-300">
				Failed
			</span>
		</div>
		<p class="text-sm text-[var(--color-text-secondary)]">Plugin produced no results.</p>
	</div>
{:else}
	<a
		href={sha256
			? `/samples/${sha256}/runs/${taskId}/p/${encodeURIComponent(view.plugin_name)}`
			: `/submissions/${taskId}/p/${encodeURIComponent(view.plugin_name)}`}
		class="group flex flex-col gap-3 rounded-2xl bg-[var(--color-bg-secondary)] p-5 transition-colors hover:bg-[var(--color-bg-tertiary)]"
	>
		<div class="flex items-start justify-between gap-3">
			<div class="min-w-0">
				<div class="flex items-baseline gap-2">
					<span class="truncate text-sm font-semibold text-[var(--color-text-primary)]">
						{displayName}
					</span>
					{#if report?.plugin.version}
						<span class="shrink-0 text-xs text-[var(--color-text-secondary)]"
							>v{report.plugin.version}</span
						>
					{/if}
				</div>
			</div>
			<div class="shrink-0">
				<ScoreBadge
					score={report?.verdict?.score ?? 0}
					classification={report?.verdict?.classification}
				/>
			</div>
		</div>

		{#if report?.summary}
			<p class="line-clamp-2 text-sm text-[var(--color-text-secondary)]">{report.summary}</p>
		{/if}

		<div
			class="mt-auto flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-[var(--color-text-secondary)]"
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
{/if}
