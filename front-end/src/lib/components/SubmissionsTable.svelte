<script lang="ts">
	import type { Task } from '$lib/api/types';
	import { splitDateTime, taskStatusLabel, isTerminalStatus } from '$lib/api/format';
	import PlatformLabel from '$lib/components/ui/PlatformLabel.svelte';

	interface Props {
		tasks: Task[];
	}

	let { tasks }: Props = $props();

	function statusClasses(status: Task['status']): string {
		if (status === 'completed') return 'bg-[var(--color-accent)]/20 text-[var(--color-accent)]';
		if (status === 'failed') return 'bg-red-500/20 text-red-300';
		if (status === 'cancelled')
			return 'bg-[var(--color-text-secondary)]/20 text-[var(--color-text-secondary)]';
		return 'bg-amber-500/20 text-amber-200';
	}
</script>

<div class="overflow-hidden rounded-xl bg-[var(--color-bg-secondary)]">
	<div
		class="grid grid-cols-[180px_1fr_120px_140px_200px_150px] gap-4 border-b border-[var(--color-border)] px-8 py-4"
	>
		<div class="text-sm font-medium text-[var(--color-text-secondary)]">Date</div>
		<div class="text-sm font-medium text-[var(--color-text-secondary)]">Target</div>
		<div class="text-sm font-medium text-[var(--color-text-secondary)]">Status</div>
		<div class="text-sm font-medium text-[var(--color-text-secondary)]">Platform</div>
		<div class="text-sm font-medium text-[var(--color-text-secondary)]">Tags</div>
		<div class="text-sm font-medium text-[var(--color-text-secondary)]">Action</div>
	</div>

	{#if tasks.length === 0}
		<div class="p-8 text-center text-sm text-[var(--color-text-secondary)]">
			No submissions yet.
		</div>
	{:else}
		{#each tasks as task (task.id)}
			{@const dt = splitDateTime(task.created_on)}
			<div
				class="grid grid-cols-[180px_1fr_120px_140px_200px_150px] items-center gap-4 border-b border-[var(--color-border)] px-8 py-3 transition-colors last:border-b-0 hover:bg-[var(--color-bg-tertiary)]"
			>
				<div class="flex flex-col gap-1">
					<span class="text-sm font-medium text-[var(--color-text-primary)]">{dt.date}</span>
					<span class="text-sm text-[var(--color-text-secondary)]">{dt.time}</span>
				</div>

				<div class="min-w-0 truncate text-sm text-[var(--color-text-primary)]" title={task.target}>
					{task.target}
				</div>

				<div class="flex items-center">
					<span
						class="whitespace-nowrap rounded px-3 py-1 text-xs font-medium {statusClasses(
							task.status
						)}"
					>
						{taskStatusLabel(task.status)}
					</span>
				</div>

				<div class="text-sm text-[var(--color-text-primary)]"><PlatformLabel platform={task.platform} /></div>

				<div class="flex flex-wrap items-center gap-1">
					{#if task.tags && task.tags.length > 0}
						{#each task.tags as tag}
							<span
								class="rounded bg-[var(--color-bg-tertiary)] px-2 py-0.5 text-xs text-[var(--color-text-secondary)]"
								>{tag}</span
							>
						{/each}
					{:else}
						<span class="text-xs text-[var(--color-text-secondary)]">—</span>
					{/if}
				</div>

				<div>
					<a
						href="/submissions/{task.id}"
						class="inline-flex items-center gap-2 rounded-lg bg-[var(--color-bg-card)] px-4 py-2 text-sm text-[var(--color-text-primary)] transition-colors hover:bg-[var(--color-bg-tertiary)]"
					>
						{isTerminalStatus(task.status) ? 'See result' : 'View'}
					</a>
				</div>
			</div>
		{/each}
	{/if}
</div>
