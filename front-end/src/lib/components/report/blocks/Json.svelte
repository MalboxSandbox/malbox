<script lang="ts">
	interface Props {
		data: unknown;
		collapsed?: boolean;
	}
	let { data, collapsed = false }: Props = $props();
	const pretty = $derived(JSON.stringify(data, null, 2));
</script>

<details class="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-primary)]" open={!collapsed}>
	<summary class="cursor-pointer px-4 py-2 font-mono text-xs text-[var(--color-text-secondary)]">
		JSON
	</summary>
	<pre
		class="overflow-x-auto border-t border-[var(--color-border)] p-4 font-mono text-xs text-[var(--color-text-primary)]">{pretty}</pre>
</details>
