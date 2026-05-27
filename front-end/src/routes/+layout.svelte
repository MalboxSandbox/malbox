<script lang="ts">
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import { auth } from '$lib/stores/auth.svelte';
	import { navigating } from '$app/stores';
	import { onMount } from 'svelte';
	import Toasts from '$lib/components/Toasts.svelte';

	let { children } = $props();

	onMount(() => {
		auth.checkAuth();
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

{#if $navigating}
	<div class="fixed left-0 top-0 z-50 h-0.5 w-full overflow-hidden bg-[var(--color-bg-tertiary)]">
		<div class="nav-progress h-full bg-[var(--color-accent)]"></div>
	</div>
{/if}

{@render children?.()}

<Toasts />

<style>
	.nav-progress {
		animation: nav-slide 2s ease-in-out infinite;
	}
	@keyframes nav-slide {
		0% {
			width: 0%;
			margin-left: 0%;
		}
		50% {
			width: 40%;
			margin-left: 30%;
		}
		100% {
			width: 0%;
			margin-left: 100%;
		}
	}
</style>
