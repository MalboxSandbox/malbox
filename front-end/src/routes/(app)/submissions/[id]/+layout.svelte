<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { startPolling } from '$lib/api/polling';
	import { isTerminalStatus } from '$lib/api/format';
	import { isApiError } from '$lib/api/errors';
	import { reportStore } from '$lib/stores/report.svelte';
	import type { Snippet } from 'svelte';
	import type { LayoutData } from './$types';

	interface Props {
		data: LayoutData;
		children: Snippet;
	}
	let { data, children }: Props = $props();

	let reportError = $state<string | null>(null);
	let pollingError = $state<string | null>(null);

	$effect(() => {
		reportError = null;
		reportStore.set(null);
		data.report.then(
			(r) => {
				reportStore.set(r);
			},
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
				await invalidate('malbox:report');
				pollingError = null;
			},
			{
				intervalMs: 2000,
				pauseWhenHidden: true,
				onError: (err) => {
					pollingError = isApiError(err) ? err.message : 'Connection lost, retrying…';
				}
			}
		);
		return stop;
	});
</script>

<div class="mx-auto max-w-7xl space-y-6">
	<div class="text-sm">
		<span class="text-[var(--color-text-secondary)]">Submission :</span>
		<span class="text-[var(--color-text-primary)]">{data.task.target}</span>
	</div>

	{#if pollingError}
		<div class="rounded-lg border-l-2 border-amber-500 bg-amber-500/10 p-3 text-sm text-amber-200">
			{pollingError}
		</div>
	{/if}

	{#if reportError}
		<div class="rounded-lg border-l-2 border-red-500 bg-red-500/10 p-4 text-sm text-red-200">
			{reportError}
		</div>
	{/if}

	{@render children()}
</div>
