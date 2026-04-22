<script lang="ts">
	import type { GraphEdge, GraphNode } from '$lib/api/types';

	interface Props {
		nodes: GraphNode[];
		edges: GraphEdge[];
	}
	let { nodes, edges }: Props = $props();
</script>

<div class="space-y-3">
	<p class="text-xs italic text-[var(--color-text-secondary)]">
		Interactive graph rendering coming soon. Listing {nodes.length} nodes and {edges.length} edges.
	</p>
	<div class="grid gap-3 md:grid-cols-2">
		<details class="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-tertiary)]">
			<summary class="cursor-pointer px-3 py-2 text-sm text-[var(--color-text-secondary)]">
				Nodes ({nodes.length})
			</summary>
			<ul class="space-y-1 border-t border-[var(--color-border)] p-3 text-xs">
				{#each nodes as n (n.id)}
					<li class="flex gap-2">
						<span class="font-mono text-[var(--color-text-secondary)]">{n.id}</span>
						<span class="text-[var(--color-text-primary)]">{n.label}</span>
					</li>
				{/each}
			</ul>
		</details>
		<details class="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-tertiary)]">
			<summary class="cursor-pointer px-3 py-2 text-sm text-[var(--color-text-secondary)]">
				Edges ({edges.length})
			</summary>
			<ul class="space-y-1 border-t border-[var(--color-border)] p-3 text-xs">
				{#each edges as e, i (i)}
					<li class="flex gap-2 font-mono">
						<span class="text-[var(--color-text-secondary)]">{e.from}</span>
						<span class="text-[var(--color-text-primary)]">→</span>
						<span class="text-[var(--color-text-secondary)]">{e.to}</span>
						{#if e.label}<span class="text-[var(--color-text-primary)]">· {e.label}</span>{/if}
					</li>
				{/each}
			</ul>
		</details>
	</div>
</div>
