<script lang="ts">
	import type { Indicator } from '$lib/api/types';
	import { contextmenu } from '$lib/actions/contextmenu';
	import CopyButton from './CopyButton.svelte';

	interface Props {
		indicators: Indicator[];
	}

	let { indicators }: Props = $props();

	const groups = $derived.by(() => {
		// eslint-disable-next-line svelte/prefer-svelte-reactivity -- local computation variable
		const map = new Map<string, Indicator[]>();
		for (const ind of indicators) {
			const existing = map.get(ind.kind);
			if (existing) {
				existing.push(ind);
			} else {
				map.set(ind.kind, [ind]);
			}
		}
		return Array.from(map.entries()).map(([kind, items]) => ({ kind, items }));
	});

	let activeKind = $state<string | null>(null);

	const currentKind = $derived(activeKind ?? groups[0]?.kind ?? null);

	const currentItems = $derived(groups.find((g) => g.kind === currentKind)?.items ?? []);
</script>

{#if groups.length > 0}
	<div class="space-y-3">
		<div class="inline-flex gap-0.5 rounded-[10px] bg-[var(--color-bg-secondary)] p-[3px]">
			{#each groups as group (group.kind)}
				<button
					type="button"
					class="rounded-lg px-2.5 py-1 text-xs font-medium transition-colors {currentKind ===
					group.kind
						? 'bg-[var(--color-accent)] text-white'
						: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
					onclick={() => (activeKind = group.kind)}
				>
					{group.kind}
					<span class="ml-1 opacity-70">{group.items.length}</span>
				</button>
			{/each}
		</div>

		<div class="space-y-0.5">
			{#each currentItems as indicator, i (indicator.kind + ':' + i)}
				<div
					class="flex items-center justify-between rounded bg-white/[0.02] px-2.5 py-1.5 text-xs"
				>
					<span
						class="min-w-0 break-all font-mono text-[11px] text-[var(--color-text-primary)]"
						use:contextmenu={{ type: 'indicator', value: indicator.value, subtype: indicator.kind }}
					>
						{indicator.value}
					</span>
					<CopyButton text={indicator.value} />
				</div>
			{/each}
		</div>
	</div>
{/if}
