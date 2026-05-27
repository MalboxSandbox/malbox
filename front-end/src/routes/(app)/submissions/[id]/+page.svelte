<script lang="ts">
	import AggregateSummary from '$lib/components/report/AggregateSummary.svelte';
	import { reportStore } from '$lib/stores/report.svelte';
	import type { PageData } from './$types';

	interface Props {
		data: PageData;
	}
	let { data }: Props = $props();
</script>

<AggregateSummary task={data.task} report={reportStore.current} />
