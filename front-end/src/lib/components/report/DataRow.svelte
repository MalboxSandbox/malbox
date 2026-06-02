<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { ContextPayload } from '$lib/components/context-menu/types';
	import { contextmenu } from '$lib/actions/contextmenu';
	import CopyButton from './CopyButton.svelte';

	interface Props {
		label: string;
		value?: string;
		copyable?: boolean;
		/** Enables the right-click context menu on the value. The row's `value` is injected automatically. */
		context?: Omit<ContextPayload, 'value'>;
		children?: Snippet;
	}

	let { label, value, copyable = false, context, children }: Props = $props();

	const payload = $derived(
		context && value !== undefined ? ({ ...context, value } as ContextPayload) : undefined
	);
</script>

<div class="flex items-center gap-3 text-sm">
	<span class="min-w-20 shrink-0 text-[var(--color-text-secondary)]">{label}</span>
	{#if children}
		<span class="flex min-w-0 items-center gap-1.5">{@render children()}</span>
	{:else if value !== undefined}
		<span class="flex min-w-0 items-center gap-1.5">
			<span class="min-w-0 truncate text-[var(--color-text-primary)]" use:contextmenu={payload}
				>{value}</span
			>
			{#if copyable && value}
				<CopyButton text={value} />
			{/if}
		</span>
	{/if}
</div>
