<script lang="ts">
	import { severityDotClass } from '../styles';
	import type { TimelineEvent } from '$lib/api/types';

	interface Props {
		events: TimelineEvent[];
	}
	let { events }: Props = $props();
</script>

<ol class="relative space-y-4 border-l border-[var(--color-border)] pl-5">
	{#each events as e, i (i)}
		<li class="relative">
			<span class="absolute -left-[26px] top-1 h-2 w-2 rounded-full {severityDotClass(e.severity)}"></span>
			<div class="space-y-1">
				<div class="font-mono text-xs text-[var(--color-text-secondary)]">{e.ts}</div>
				<div class="text-sm text-[var(--color-text-primary)]">{e.label}</div>
				{#if e.meta && e.meta !== null}
					<div class="font-mono text-[10px] text-[var(--color-text-secondary)]">
						{typeof e.meta === 'object' ? JSON.stringify(e.meta) : String(e.meta)}
					</div>
				{/if}
			</div>
		</li>
	{/each}
</ol>
