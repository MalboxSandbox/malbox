<script lang="ts">
	import CopyButton from '$lib/components/ui/CopyButton.svelte';
	import type { Indicator } from '$lib/api/types';

	interface Props {
		items: Indicator[];
	}
	let { items }: Props = $props();

	const grouped = $derived.by(() => {
		const m = new Map<string, Indicator[]>();
		for (const ind of items) {
			if (!m.has(ind.kind)) m.set(ind.kind, []);
			m.get(ind.kind)!.push(ind);
		}
		return Array.from(m.entries()).sort((a, b) => a[0].localeCompare(b[0]));
	});
</script>

{#if items.length === 0}
	<p class="text-sm text-[var(--color-text-secondary)]">No indicators.</p>
{:else}
	<div class="space-y-4">
		{#each grouped as [kind, list] (kind)}
			<div class="space-y-2">
				<div class="text-xs font-medium uppercase tracking-wide text-[var(--color-text-secondary)]">
					{kind}
					<span class="ml-1 opacity-70">({list.length})</span>
				</div>
				<ul class="space-y-1">
					{#each list as ind, i (kind + ':' + i)}
						<li
							class="flex items-center gap-3 rounded-lg bg-[var(--color-bg-tertiary)] px-3 py-2"
						>
							<span class="min-w-0 flex-1 break-all font-mono text-xs text-[var(--color-text-primary)]">
								{ind.value}
							</span>
							<CopyButton value={ind.value} size="sm" />
							{#if ind.context}
								<span class="shrink-0 text-xs text-[var(--color-text-secondary)]">{ind.context}</span>
							{/if}
						</li>
					{/each}
				</ul>
			</div>
		{/each}
	</div>
{/if}
