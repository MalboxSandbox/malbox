<script lang="ts">
	interface Props {
		language: string;
		text: string;
	}
	let { language, text }: Props = $props();
</script>

<div class="space-y-2">
	{#if language}
		<span
			class="inline-block rounded bg-[var(--color-bg-tertiary)] px-2 py-0.5 font-mono text-[10px] uppercase text-[var(--color-text-secondary)]"
		>
			{language}
		</span>
	{/if}
	<pre
		class="overflow-x-auto rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-primary)] p-4 font-mono text-xs text-[var(--color-text-primary)]">{text}</pre>
</div>
