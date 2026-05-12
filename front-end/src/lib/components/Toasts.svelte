<script lang="ts">
	import { toasts } from '$lib/stores/toasts.svelte';
	import { fly, fade } from 'svelte/transition';
</script>

<div class="fixed right-4 top-4 z-50 flex w-[340px] max-w-[calc(100vw-2rem)] flex-col gap-2.5">
	{#each toasts.list as toast (toast.id)}
		<div
			class="relative overflow-hidden rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-secondary)] px-4 py-3 shadow-lg shadow-black/25"
			role="status"
			in:fly={{ x: 40, duration: 250 }}
			out:fade={{ duration: 150 }}
		>
			<div class="absolute top-0 left-0 h-full w-[3px] bg-[var(--color-accent)]"></div>
			<p class="text-sm leading-relaxed text-[var(--color-text-primary)]">
				{toast.message}
			</p>
		</div>
	{/each}
</div>
