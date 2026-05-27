<script lang="ts">
	import AppLayout from '$lib/layouts/AppLayout.svelte';
	import ContextMenu from '$lib/components/context-menu/ContextMenu.svelte';
	import ValuePreview from '$lib/components/ValuePreview.svelte';
	import { transformStore } from '$lib/stores/transforms.svelte';

	let { children }: { children?: import('svelte').Snippet } = $props();

	$effect(() => {
		transformStore.loadCustomTransforms(fetch);
	});
</script>

<AppLayout>
	{@render children?.()}
</AppLayout>

<ContextMenu />
<ValuePreview />
