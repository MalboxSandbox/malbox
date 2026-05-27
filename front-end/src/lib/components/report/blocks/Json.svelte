<script lang="ts">
	import CodeViewer from '$lib/components/CodeViewer.svelte';

	interface Props {
		data: unknown;
		collapsed?: boolean;
	}
	let { data, collapsed = false }: Props = $props();
	const pretty = $derived(JSON.stringify(data, null, 2));

	let open = $state(!collapsed);
</script>

<div class="rounded-lg bg-[var(--color-bg-primary)]">
	<button
		type="button"
		class="flex w-full items-center gap-2 px-4 py-2.5 text-left font-mono text-xs text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text-primary)]"
		onclick={() => (open = !open)}
	>
		<svg
			xmlns="http://www.w3.org/2000/svg"
			viewBox="0 0 20 20"
			fill="currentColor"
			class="size-3.5 shrink-0 transition-transform duration-150 {open ? 'rotate-90' : ''}"
		>
			<path
				fill-rule="evenodd"
				d="M7.21 14.77a.75.75 0 0 1 .02-1.06L11.168 10 7.23 6.29a.75.75 0 1 1 1.04-1.08l4.5 4.25a.75.75 0 0 1 0 1.08l-4.5 4.25a.75.75 0 0 1-1.06-.02Z"
				clip-rule="evenodd"
			/>
		</svg>
		<span>JSON</span>
	</button>
	{#if open}
		<div class="border-t border-[var(--color-border)]/30">
			<CodeViewer code={pretty} language="json" />
		</div>
	{/if}
</div>
