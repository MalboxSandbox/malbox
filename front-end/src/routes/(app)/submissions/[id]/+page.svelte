<script lang="ts">
	import AggregateSummary from '$lib/components/report/AggregateSummary.svelte';
	import type { PageData } from './$types';

	interface Props {
		data: PageData;
	}
	let { data }: Props = $props();
</script>

<AggregateSummary data={data.report} />
