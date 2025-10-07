<script lang="ts">
	import AppLayout from '$lib/layouts/AppLayout.svelte';

	let { children }: { children?: import('svelte').Snippet } = $props();
</script>

<AppLayout>
	{@render children?.()}
</AppLayout>
