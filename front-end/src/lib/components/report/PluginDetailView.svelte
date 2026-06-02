<script lang="ts">
	import type { PluginReportView, Sample } from '$lib/api/types';
	import { classificationClasses } from './styles';
	import PluginReport from './PluginReport.svelte';
	import ScoreBadge from './ScoreBadge.svelte';
	import VerdictBadge from './VerdictBadge.svelte';

	interface Props {
		view: PluginReportView;
		sample?: Sample;
		onback: () => void;
	}

	let { view, sample, onback }: Props = $props();

	const report = $derived(view.report);
	const displayName = $derived(
		report?.plugin.display_name ?? report?.plugin.id ?? view.plugin_name
	);
	const classification = $derived(report?.verdict?.classification);
	const cls = $derived(classificationClasses(classification));
</script>

<div>
	<button
		type="button"
		class="mb-4 inline-flex items-center gap-1.5 text-xs text-[var(--color-accent)] hover:underline"
		onclick={onback}
	>
		<svg
			xmlns="http://www.w3.org/2000/svg"
			viewBox="0 0 20 20"
			fill="currentColor"
			class="size-3.5"
		>
			<path
				fill-rule="evenodd"
				d="M11.78 5.22a.75.75 0 0 1 0 1.06L8.06 10l3.72 3.72a.75.75 0 1 1-1.06 1.06l-4.25-4.25a.75.75 0 0 1 0-1.06l4.25-4.25a.75.75 0 0 1 1.06 0Z"
				clip-rule="evenodd"
			/>
		</svg>
		Back to report
	</button>

	<div class="mb-5 flex items-center justify-between">
		<div class="flex items-center gap-3">
			<div
				class="flex size-9 items-center justify-center rounded-lg {cls.tint ||
					'bg-[var(--color-bg-card)]'}"
			>
				<svg
					xmlns="http://www.w3.org/2000/svg"
					viewBox="0 0 20 20"
					fill="currentColor"
					class="size-4 text-[var(--color-text-secondary)]"
				>
					<path
						fill-rule="evenodd"
						d="M4.5 2A1.5 1.5 0 0 0 3 3.5v13A1.5 1.5 0 0 0 4.5 18h11a1.5 1.5 0 0 0 1.5-1.5V7.621a1.5 1.5 0 0 0-.44-1.06l-4.12-4.122A1.5 1.5 0 0 0 11.378 2H4.5Zm4.531 6.22a.75.75 0 0 1 0 1.06L7.28 11l1.751 1.72a.75.75 0 1 1-1.06 1.06l-2.282-2.25a.75.75 0 0 1 0-1.06l2.282-2.25a.75.75 0 0 1 1.06 0Zm2.938 0a.75.75 0 0 1 1.06 0l2.282 2.25a.75.75 0 0 1 0 1.06l-2.282 2.25a.75.75 0 0 1-1.06-1.06L12.72 11l-1.751-1.72a.75.75 0 0 1 0-1.06Z"
						clip-rule="evenodd"
					/>
				</svg>
			</div>
			<div>
				<div class="flex items-center gap-2">
					<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">
						{displayName}
					</h2>
					{#if report?.plugin.version}
						<span
							class="rounded bg-[var(--color-bg-tertiary)] px-1.5 py-0.5 text-[10px] text-[var(--color-text-secondary)]"
						>
							v{report.plugin.version}
						</span>
					{/if}
				</div>
				<div class="mt-1 flex items-center gap-2">
					{#if classification}
						<VerdictBadge {classification} size="sm" />
					{/if}
					{#if report?.summary}
						<span class="text-xs text-[var(--color-text-secondary)]">
							{report.summary}
						</span>
					{/if}
				</div>
			</div>
		</div>
		{#if report?.verdict?.score != null}
			<ScoreBadge score={report.verdict.score} classification={report.verdict.classification} />
		{/if}
	</div>

	<div class="mb-5 h-px bg-[var(--color-border)]"></div>

	<PluginReport {view} {sample} />
</div>
