<script lang="ts">
	import type { Ttp } from '$lib/api/types';

	interface Props {
		items: Ttp[];
	}
	let { items }: Props = $props();

	function attackUrl(id: string): string {
		// Strip sub-technique suffix for the URL (MITRE pages accept full id too,
		// but the canonical landing page is at the parent technique).
		const parent = id.split('.')[0];
		return `https://attack.mitre.org/techniques/${parent}/`;
	}
</script>

{#if items.length === 0}
	<p class="text-sm text-[var(--color-text-secondary)]">No TTPs.</p>
{:else}
	<ul class="space-y-2">
		{#each items as t (t.id)}
			<li
				class="flex flex-wrap items-baseline gap-3 rounded border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm"
			>
				<a
					href={attackUrl(t.id)}
					target="_blank"
					rel="noopener"
					class="shrink-0 rounded bg-[var(--color-bg-card)] px-2 py-0.5 font-mono text-xs text-[var(--color-accent)] hover:underline"
				>
					{t.id}
				</a>
				<span class="text-[var(--color-text-primary)]">{t.name}</span>
				{#if t.evidence}
					<span class="w-full text-xs text-[var(--color-text-secondary)]">{t.evidence}</span>
				{/if}
			</li>
		{/each}
	</ul>
{/if}
