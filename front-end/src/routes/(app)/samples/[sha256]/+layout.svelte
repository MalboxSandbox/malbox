<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { startPolling } from '$lib/api/polling';
	import type { Snippet } from 'svelte';
	import type { LayoutData } from './$types';

	interface Props {
		data: LayoutData;
		children: Snippet;
	}
	let { data, children }: Props = $props();

	const hasPendingTasks = $derived(
		data.overview.tasks.some((t) => !['completed', 'failed', 'canceled'].includes(t.status))
	);

	$effect(() => {
		if (!hasPendingTasks) return;
		const stop = startPolling(
			async () => {
				await invalidate('malbox:sample');
			},
			{ intervalMs: 3000, pauseWhenHidden: true }
		);
		return stop;
	});
</script>

<div class="mx-auto max-w-7xl">
	{@render children()}
</div>
