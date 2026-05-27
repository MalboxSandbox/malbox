<script lang="ts">
	import type { PipelineStep } from '$lib/transforms';

	interface Props {
		steps: PipelineStep[];
		activeIndex: number;
		onRewind: (index: number) => void;
		onRemoveStep: (index: number) => void;
	}

	let { steps, activeIndex, onRewind, onRemoveStep }: Props = $props();
</script>

{#if steps.length > 0}
	<div
		class="flex flex-wrap items-center gap-1.5 rounded-lg bg-[var(--color-bg-tertiary)] px-3 py-2 text-xs"
	>
		<button
			onclick={() => onRewind(-1)}
			class="rounded px-2 py-1 transition-colors {activeIndex === -1
				? 'bg-[var(--color-accent)] text-white'
				: 'text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)] hover:text-[var(--color-text-primary)]'}"
		>
			raw
		</button>

		{#each steps as step, i (i)}
			<span class="text-[var(--color-text-secondary)]">&rarr;</span>
			<span class="group inline-flex items-center gap-1">
				<button
					onclick={() => onRewind(i)}
					class="rounded px-2 py-1 transition-colors {activeIndex === i
						? 'bg-[var(--color-accent)] text-white'
						: 'text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)] hover:text-[var(--color-text-primary)]'}"
				>
					{step.transformId}
				</button>
				{#if step.source === 'auto-detect'}
					<span
						class="rounded bg-[var(--color-bg-secondary)] px-1 py-0.5 text-[10px] text-[var(--color-text-secondary)]"
					>
						auto
					</span>
				{/if}
				<button
					onclick={() => onRemoveStep(i)}
					class="hidden rounded px-1 py-0.5 text-[var(--color-text-secondary)] hover:bg-[var(--color-error)]/20 hover:text-[var(--color-error)] group-hover:inline-block"
				>
					&times;
				</button>
			</span>
		{/each}
	</div>
{/if}
