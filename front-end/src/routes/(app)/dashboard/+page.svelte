<script lang="ts">
	import FileUpload from '$lib/components/FileUpload.svelte';
	import SubmissionStats from '$lib/components/SubmissionStats.svelte';
	import SystemStats from '$lib/components/SystemStats.svelte';
	import { invalidate } from '$app/navigation';
	import { startPolling } from '$lib/api/polling';
	import type { PageData } from './$types';

	interface Props {
		data: PageData;
	}

	let { data }: Props = $props();

	$effect(() => {
		const stop = startPolling(() => invalidate('malbox:tasks'), {
			intervalMs: 10_000,
			pauseWhenHidden: true
		});
		return stop;
	});
</script>

<div class="mx-auto flex max-w-7xl flex-col gap-8">
	<div class="space-y-3 text-center">
		<h1 class="text-3xl font-semibold text-[var(--color-text-primary)]">
			Upload your File, URL or Hash
		</h1>
		<p class="text-[var(--color-text-secondary)]">
			Upload your sample and get started with analysis, or just insert a hash or URL to match with
			your IOC database!
		</p>
	</div>

	<FileUpload />

	<div class="grid grid-cols-1 gap-8 lg:grid-cols-2">
		<SubmissionStats counts={data.counts} />
		<SystemStats machines={data.machines} />
	</div>
</div>
