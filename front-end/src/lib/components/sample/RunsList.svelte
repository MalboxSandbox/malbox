<script lang="ts">
	import type { TaskSummary } from '$lib/api/types';
	import RunCard from './RunCard.svelte';

	interface Props {
		tasks: TaskSummary[];
		sha256: string;
	}

	let { tasks, sha256 }: Props = $props();
</script>

<div class="space-y-3 rounded-xl bg-[var(--color-bg-secondary)] p-6">
	<h2 class="text-sm font-semibold uppercase tracking-wide text-[var(--color-text-secondary)]">
		Analysis Runs ({tasks.length})
	</h2>

	{#if tasks.length === 0}
		<p class="text-sm text-[var(--color-text-secondary)]">No analysis runs yet.</p>
	{:else}
		<div class="space-y-2">
			{#each tasks as task (task.id)}
				<RunCard {task} {sha256} />
			{/each}
		</div>
	{/if}
</div>
