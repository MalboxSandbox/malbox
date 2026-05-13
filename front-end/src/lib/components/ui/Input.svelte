<script lang="ts">
	import { createLabel } from '@melt-ui/svelte';

	let {
		type = 'text',
		label = '',
		placeholder = '',
		value = $bindable(''),
		required = false,
		error = '',
		...restProps
	}: {
		type?: string;
		label?: string;
		placeholder?: string;
		value?: string;
		required?: boolean;
		error?: string;
		// eslint-disable-next-line @typescript-eslint/no-explicit-any -- rest props spread
		[key: string]: any;
	} = $props();

	const {
		elements: { root }
	} = createLabel();
</script>

<div class="space-y-2">
	{#if label}
		<label use:$root class="block text-sm font-medium text-[var(--color-text-primary)]">
			{label}
			{#if required}
				<span class="text-[var(--color-accent)]">*</span>
			{/if}
		</label>
	{/if}

	<input
		{type}
		{placeholder}
		{required}
		bind:value
		{...restProps}
		class="w-full px-4 py-3 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-lg
               text-[var(--color-text-primary)] placeholder-[var(--color-text-secondary)]
               focus:outline-none focus:border-[var(--color-accent)] focus:ring-1 focus:ring-[var(--color-accent)]
               transition-all duration-200
               disabled:opacity-50 disabled:cursor-not-allowed
               {error
			? 'border-[var(--color-error)] focus:border-[var(--color-error)] focus:ring-[var(--color-error)]'
			: ''}"
	/>

	{#if error}
		<p class="text-sm text-[var(--color-error)]">{error}</p>
	{/if}
</div>
