<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		id: string;
		title: string;
		count?: string | number;
		children?: Snippet;
	}

	let { id, title, count, children }: Props = $props();
</script>

<div id="section-{id}" class="mb-3.5 border-b border-[var(--color-border)] pb-2 pt-7">
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-2">
			<h3 class="text-sm font-semibold text-[var(--color-text-primary)]">{title}</h3>
			{#if count !== undefined}
				<span
					class="rounded-md bg-[var(--color-bg-card)] px-1.5 py-0.5 text-xs text-[var(--color-text-secondary)]"
				>
					{count}
				</span>
			{/if}
		</div>
		{#if children}
			{@render children()}
		{/if}
	</div>
</div>
