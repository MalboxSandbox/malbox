<script lang="ts">
	import PluginReport from '$lib/components/report/PluginReport.svelte';
	import type { PageData } from './$types';

	interface Props {
		data: PageData;
	}
	let { data }: Props = $props();

	const sampleSha256 = $derived(data.task.sample?.sha256 ?? '');
	const backHref = $derived(`/samples/${sampleSha256}`);
</script>

<div class="space-y-4">
	<div class="flex items-center gap-3">
		<a href={backHref} class="text-sm text-[var(--color-accent)] hover:underline">
			&larr; Back to report
		</a>
		<span class="text-sm text-[var(--color-text-secondary)]">
			{data.view.report?.plugin.display_name ?? data.view.plugin_name}
		</span>
	</div>

	<PluginReport view={data.view} sample={data.sample} />
</div>
