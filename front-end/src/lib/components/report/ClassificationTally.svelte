<script lang="ts">
	import { classificationClasses } from './styles';

	interface Props {
		classifications: Record<string, number>;
	}

	let { classifications }: Props = $props();

	const order = ['malicious', 'suspicious', 'unknown', 'clean'] as const;
	const entries = $derived(
		order.map((k) => ({ k, n: classifications[k] ?? 0 })).filter((e) => e.n > 0)
	);
	const total = $derived(entries.reduce((s, e) => s + e.n, 0));
</script>

{#if total > 0}
	<div class="space-y-2">
		<div class="flex h-2 overflow-hidden rounded-full bg-[var(--color-bg-tertiary)]">
			{#each entries as e (e.k)}
				<div
					class={classificationClasses(e.k).dot}
					style="width: {(e.n / total) * 100}%"
					title="{e.k}: {e.n}"
				></div>
			{/each}
		</div>
		<div class="flex flex-wrap gap-x-4 gap-y-1 text-xs text-[var(--color-text-secondary)]">
			{#each entries as e (e.k)}
				<span class="inline-flex items-center gap-1.5">
					<span class="h-2 w-2 rounded-full {classificationClasses(e.k).dot}"></span>
					<span>{e.n} {e.k}</span>
				</span>
			{/each}
		</div>
	</div>
{/if}
