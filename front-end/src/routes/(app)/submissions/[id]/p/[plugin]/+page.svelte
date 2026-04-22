<script lang="ts">
	import PluginReport from '$lib/components/report/PluginReport.svelte';
	import type { PageData } from './$types';

	interface Props {
		data: PageData;
	}
	let { data }: Props = $props();
</script>

<PluginReport view={data.view} />
