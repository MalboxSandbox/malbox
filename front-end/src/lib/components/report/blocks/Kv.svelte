<script lang="ts">
	import type { KvPair } from '$lib/api/types';

	interface Props {
		pairs: KvPair[];
	}
	let { pairs }: Props = $props();
</script>

<dl class="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-2 text-sm">
	{#each pairs as p, i (i)}
		<dt class="text-[var(--color-text-secondary)]">{p.key}</dt>
		<dd
			class="break-all text-[var(--color-text-primary)] {p.mono ? 'font-mono text-xs' : ''}"
		>
			{p.value}
		</dd>
	{/each}
</dl>
