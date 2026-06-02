<script lang="ts">
	import type { Indicator } from '$lib/api/types';

	interface Props {
		indicators: Indicator[];
	}

	let { indicators }: Props = $props();

	let expanded = $state(false);
	const PREVIEW_LIMIT = 10;

	const grouped = $derived.by(() => {
		const groups: Record<string, Indicator[]> = {};
		for (const ind of indicators) {
			(groups[ind.kind] ??= []).push(ind);
		}
		return groups;
	});

	const displayIndicators = $derived(expanded ? indicators : indicators.slice(0, PREVIEW_LIMIT));
</script>

{#if indicators.length > 0}
	<div class="space-y-3 rounded-xl bg-[var(--color-bg-secondary)] p-6">
		<div class="flex items-center justify-between">
			<h2 class="text-sm font-semibold uppercase tracking-wide text-[var(--color-text-secondary)]">
				Indicators ({indicators.length})
			</h2>
		</div>

		<div class="flex flex-wrap gap-2">
			{#each Object.entries(grouped) as [kind, items] (kind)}
				<span
					class="rounded bg-[var(--color-accent)]/10 px-2.5 py-1 text-xs font-medium text-[var(--color-accent)]"
				>
					{kind}: {items.length}
				</span>
			{/each}
		</div>

		<div class="space-y-1.5">
			{#each displayIndicators as ind (ind.kind + ':' + ind.value)}
				<div
					class="flex items-center gap-3 rounded-lg bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm"
				>
					<span
						class="shrink-0 rounded bg-[var(--color-bg-card)] px-2 py-0.5 text-xs font-medium text-[var(--color-text-secondary)]"
					>
						{ind.kind}
					</span>
					<span class="min-w-0 truncate font-mono text-[var(--color-text-primary)]">
						{ind.value}
					</span>
					{#if ind.context}
						<span class="shrink-0 text-xs text-[var(--color-text-secondary)]">
							{ind.context}
						</span>
					{/if}
				</div>
			{/each}
		</div>

		{#if indicators.length > PREVIEW_LIMIT}
			<button
				onclick={() => (expanded = !expanded)}
				class="text-sm text-[var(--color-accent)] hover:underline"
			>
				{expanded ? 'Show less' : `Show all ${indicators.length} indicators`}
			</button>
		{/if}
	</div>
{/if}
