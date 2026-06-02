<script lang="ts">
	import type { SampleInfo, SampleAggregate } from '$lib/api/types';
	import VerdictBadge from '$lib/components/report/VerdictBadge.svelte';
	import CopyButton from '$lib/components/ui/CopyButton.svelte';
	import { formatBytes } from '$lib/api/format';
	import type { Classification } from '$lib/api/types';

	interface Props {
		sample: SampleInfo;
		aggregate: SampleAggregate;
		sha256: string;
		onReanalyze?: () => void;
	}

	let { sample, aggregate, sha256, onReanalyze }: Props = $props();
</script>

<div
	class="flex flex-wrap items-start justify-between gap-4 rounded-xl bg-[var(--color-bg-secondary)] p-6"
>
	<div class="min-w-0 flex-1 space-y-2">
		<div class="flex items-center gap-3">
			<h1 class="truncate text-lg font-semibold text-[var(--color-text-primary)]">
				{sample.file_type}
			</h1>
			<span class="shrink-0 text-sm text-[var(--color-text-secondary)]">
				{formatBytes(sample.file_size)}
			</span>
		</div>
		<div class="flex items-center gap-2 font-mono text-xs text-[var(--color-text-secondary)]">
			<span class="truncate">{sha256}</span>
			<CopyButton value={sha256} />
		</div>
	</div>

	<div class="flex items-center gap-4">
		{#if aggregate.worst_verdict}
			<VerdictBadge
				classification={aggregate.worst_verdict as Classification}
				score={aggregate.worst_score ?? undefined}
			/>
		{:else}
			<span class="text-sm text-[var(--color-text-secondary)]">No verdict yet</span>
		{/if}

		<div class="text-center">
			<div class="text-lg font-bold text-[var(--color-text-primary)]">
				{aggregate.indicator_count + aggregate.ttp_count}
			</div>
			<div class="text-xs text-[var(--color-text-secondary)]">findings</div>
		</div>

		{#if onReanalyze}
			<button
				onclick={onReanalyze}
				class="rounded-lg bg-[var(--color-accent)]/20 px-4 py-2 text-sm font-medium text-[var(--color-accent)] transition-colors hover:bg-[var(--color-accent)]/30"
			>
				Re-analyze
			</button>
		{/if}
	</div>
</div>
