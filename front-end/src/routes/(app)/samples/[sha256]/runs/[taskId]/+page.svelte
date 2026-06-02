<script lang="ts">
	import AggregateSummary from '$lib/components/report/AggregateSummary.svelte';
	import { invalidate } from '$app/navigation';
	import { startPolling } from '$lib/api/polling';
	import { isTerminalStatus } from '$lib/api/format';
	import { isApiError } from '$lib/api/errors';
	import { reportStore } from '$lib/stores/report.svelte';
	import type { PageData } from './$types';

	interface Props {
		data: PageData;
	}
	let { data }: Props = $props();

	let reportError = $state<string | null>(null);

	$effect(() => {
		reportError = null;
		reportStore.set(null);
		data.report.then(
			(r) => reportStore.set(r),
			(err) => {
				reportError = isApiError(err) ? err.message : 'Failed to load report';
			}
		);
		return () => reportStore.set(null);
	});

	$effect(() => {
		if (isTerminalStatus(data.task.status)) return;
		const stop = startPolling(
			async () => {
				await invalidate('malbox:run-report');
			},
			{ intervalMs: 2000, pauseWhenHidden: true }
		);
		return stop;
	});
</script>

<div class="space-y-4">
	<div class="flex items-center gap-3">
		<a
			href="/samples/{data.task.sample?.sha256 ?? ''}"
			class="text-sm text-[var(--color-accent)] hover:underline"
		>
			&larr; Back to report
		</a>
		<span class="text-sm text-[var(--color-text-secondary)]">
			Run #{data.taskId}
		</span>
	</div>

	{#if reportError}
		<div class="rounded-lg border-l-2 border-red-500 bg-red-500/10 p-4 text-sm text-red-200">
			{reportError}
		</div>
	{/if}

	<AggregateSummary task={data.task} report={reportStore.current} />
</div>
