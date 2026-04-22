<script lang="ts">
	import { toasts } from '$lib/stores/toasts.svelte';
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

	async function copy(value: string) {
		try {
			await navigator.clipboard.writeText(value);
		} catch {
			toasts.push({ kind: 'error', message: 'Clipboard write failed' });
		}
	}
</script>

{#if items.length === 0}
	<p class="text-sm text-[var(--color-text-secondary)]">No indicators.</p>
{:else}
	<div class="space-y-4">
		{#each grouped as [kind, list] (kind)}
			<div class="space-y-2">
				<div class="text-xs font-medium uppercase tracking-wide text-[var(--color-text-secondary)]">
					{kind}
					<span class="ml-1 text-[var(--color-text-secondary)]/70">({list.length})</span>
				</div>
				<ul class="space-y-1">
					{#each list as ind, i (kind + ':' + i)}
						<li
							class="flex items-start gap-2 rounded border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm"
						>
							<span class="flex-1 break-all font-mono text-xs text-[var(--color-text-primary)]">
								{ind.value}
							</span>
							<button
								type="button"
								class="shrink-0 rounded px-2 py-0.5 text-xs text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-card)] hover:text-[var(--color-text-primary)]"
								title="Copy"
								onclick={() => copy(ind.value)}
							>
								Copy
							</button>
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
