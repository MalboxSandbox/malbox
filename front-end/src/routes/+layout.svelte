<script lang="ts">
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import { auth } from '$lib/stores/auth.svelte';
	import { onMount } from 'svelte';

	let { children } = $props();

	// Check auth status on app load
	onMount(() => {
		auth.checkAuth();
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

{@render children?.()}
