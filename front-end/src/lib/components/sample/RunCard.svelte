<script lang="ts">
	import type { TaskSummary } from '$lib/api/types';
	import { splitDateTime, taskStatusLabel } from '$lib/api/format';
	import PlatformLabel from '$lib/components/ui/PlatformLabel.svelte';

	interface Props {
		task: TaskSummary;
		sha256: string;
	}

	let { task, sha256 }: Props = $props();

	const dt = $derived(splitDateTime(task.created_on));

	function statusClasses(status: TaskSummary['status']): string {
		if (status === 'completed') return 'bg-[var(--color-accent)]/20 text-[var(--color-accent)]';
		if (status === 'failed') return 'bg-red-500/20 text-red-300';
		if (status === 'canceled')
			return 'bg-[var(--color-text-secondary)]/20 text-[var(--color-text-secondary)]';
		return 'bg-amber-500/20 text-amber-200';
	}
</script>

<a
	href="/samples/{sha256}/runs/{task.id}"
	class="flex items-center justify-between gap-4 rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 transition-colors hover:bg-[var(--color-bg-card)]"
>
	<div class="flex items-center gap-4">
		<span class="text-sm font-medium text-[var(--color-text-primary)]">#{task.id}</span>
		<PlatformLabel platform={task.platform} />
		<span class="text-xs text-[var(--color-text-secondary)]">
			{task.plugins.length} plugin{task.plugins.length !== 1 ? 's' : ''} &bull; {task.timeout}s
		</span>
	</div>

	<div class="flex items-center gap-3">
		<span
			class="whitespace-nowrap rounded px-2.5 py-1 text-xs font-medium {statusClasses(task.status)}"
		>
			{taskStatusLabel(task.status)}
		</span>
		<span class="text-xs text-[var(--color-text-secondary)]">
			{dt.date}
			{dt.time}
		</span>
	</div>
</a>
