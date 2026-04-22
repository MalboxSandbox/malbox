<script lang="ts">
	interface Props {
		level: number;
		text: string;
	}
	let { level, text }: Props = $props();
	const clamped = $derived(Math.max(2, Math.min(4, level)));
</script>

{#if clamped === 2}
	<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">{text}</h2>
{:else if clamped === 3}
	<h3 class="text-base font-semibold text-[var(--color-text-primary)]">{text}</h3>
{:else}
	<h4 class="text-sm font-semibold text-[var(--color-text-primary)]">{text}</h4>
{/if}
