<script lang="ts">
	import CopyButton from '$lib/components/ui/CopyButton.svelte';

	interface Props {
		language: string;
		text: string;
	}
	let { language, text }: Props = $props();
</script>

<div class="relative overflow-hidden rounded-lg bg-[var(--color-bg-primary)]">
	<div class="flex items-center justify-between px-4 py-2">
		{#if language}
			<span class="font-mono text-[10px] uppercase text-[var(--color-text-secondary)]">
				{language}
			</span>
		{:else}
			<span></span>
		{/if}
		<CopyButton value={text} size="sm" />
	</div>
	<pre
		class="overflow-x-auto border-t border-[var(--color-border)]/30 px-4 py-3 font-mono text-xs leading-relaxed text-[var(--color-text-primary)]">{text}</pre>
</div>
