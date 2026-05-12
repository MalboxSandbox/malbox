<script lang="ts">
	import Json from './Json.svelte';
	import type { Block } from '$lib/api/types';

	interface Props {
		// We receive the full block because its shape is, by definition, unknown.
		block: Block | { type: string; [k: string]: unknown };
	}
	let { block }: Props = $props();
</script>

<div class="space-y-2 rounded-lg bg-[var(--color-bg-tertiary)] p-4">
	<div class="text-xs text-[var(--color-text-secondary)]">
		Unsupported block type:
		<span class="font-mono text-[var(--color-text-primary)]">{(block as { type: string }).type}</span>
	</div>
	<Json data={block} collapsed />
</div>
