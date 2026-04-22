<script lang="ts">
	import type { TreeNode } from '$lib/api/types';
	import Self from './Tree.svelte';

	interface Props {
		nodes: TreeNode[];
	}
	let { nodes }: Props = $props();
</script>

<ul class="space-y-1 pl-4">
	{#each nodes as n, i (i)}
		<li>
			{#if n.children && n.children.length > 0}
				<details open>
					<summary class="cursor-pointer text-sm text-[var(--color-text-primary)]">
						{n.label}
					</summary>
					<Self nodes={n.children} />
				</details>
			{:else}
				<span class="text-sm text-[var(--color-text-primary)]">{n.label}</span>
			{/if}
			{#if n.meta && n.meta !== null}
				<span class="ml-2 font-mono text-[10px] text-[var(--color-text-secondary)]">
					{typeof n.meta === 'object' ? JSON.stringify(n.meta) : String(n.meta)}
				</span>
			{/if}
		</li>
	{/each}
</ul>
