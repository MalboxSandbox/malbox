<script lang="ts">
	import Sidebar from '$lib/components/Sidebar.svelte';
	import { sidebar } from '$lib/stores/sidebar.svelte';

	let { children }: { children?: import('svelte').Snippet } = $props();
</script>

<div class="flex h-screen bg-[var(--color-bg-primary)]">
	<Sidebar />

	<main class="flex-1 overflow-auto transition-[margin-left] duration-300 ease-out {sidebar.collapsed ? 'ml-[72px]' : 'ml-64'}">
		<div class="p-8">
			{@render children?.()}
		</div>
	</main>
</div>
