<script lang="ts">
	import Sidebar from '$lib/components/Sidebar.svelte';

	let { children }: { children?: import('svelte').Snippet } = $props();
</script>

<div class="flex h-screen bg-[var(--color-bg-primary)]">
	<Sidebar />

	<main class="flex-1 ml-64 overflow-auto">
		<div class="p-8">
			{@render children?.()}
		</div>
	</main>
</div>
