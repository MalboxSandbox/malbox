<script lang="ts">
	import type { Ttp } from '$lib/api/types';

	interface Props {
		items: Ttp[];
	}
	let { items }: Props = $props();

	function attackUrl(id: string): string {
		const parent = id.split('.')[0];
		return `https://attack.mitre.org/techniques/${parent}/`;
	}
</script>

{#if items.length === 0}
	<p class="text-sm text-[var(--color-text-secondary)]">No TTPs.</p>
{:else}
	<ul class="space-y-1">
		{#each items as t (t.id)}
			<li class="flex flex-wrap items-baseline gap-3 rounded-lg bg-[var(--color-bg-tertiary)] px-3 py-2.5">
				<a
					href={attackUrl(t.id)}
					target="_blank"
					rel="noopener"
					class="inline-flex shrink-0 items-center gap-1.5 rounded-md bg-[var(--color-bg-card)] px-2 py-0.5 font-mono text-xs text-[var(--color-accent)] transition-colors hover:bg-[var(--color-accent)]/10"
				>
					{t.id}
					<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor" class="size-2.5 opacity-60">
						<path d="M6.22 8.72a.75.75 0 0 0 1.06 1.06l5.22-5.22v1.69a.75.75 0 0 0 1.5 0v-3.5a.75.75 0 0 0-.75-.75h-3.5a.75.75 0 0 0 0 1.5h1.69L6.22 8.72Z" />
						<path d="M3.5 6.75c0-.69.56-1.25 1.25-1.25H7A.75.75 0 0 0 7 4H4.75A2.75 2.75 0 0 0 2 6.75v4.5A2.75 2.75 0 0 0 4.75 14h4.5A2.75 2.75 0 0 0 12 11.25V9a.75.75 0 0 0-1.5 0v2.25c0 .69-.56 1.25-1.25 1.25h-4.5c-.69 0-1.25-.56-1.25-1.25v-4.5Z" />
					</svg>
				</a>
				<span class="text-sm text-[var(--color-text-primary)]">{t.name}</span>
				{#if t.evidence}
					<span class="w-full text-xs text-[var(--color-text-secondary)]">{t.evidence}</span>
				{/if}
			</li>
		{/each}
	</ul>
{/if}
