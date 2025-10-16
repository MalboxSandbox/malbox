<script lang="ts">
	let {
		type = 'button',
		loading = false,
		disabled = false,
		variant = 'primary',
		children,
		...restProps
	}: {
		type?: 'button' | 'submit' | 'reset';
		loading?: boolean;
		disabled?: boolean;
		variant?: 'primary' | 'secondary';
		children?: import('svelte').Snippet;
		[key: string]: any;
	} = $props();

	const isDisabled = $derived(disabled || loading);

	const variantClasses = $derived(
		variant === 'primary'
			? 'bg-[var(--color-accent)] text-[var(--color-text-primary)] hover:bg-[var(--color-accent-hover)] disabled:bg-[var(--color-accent)]/50'
			: 'bg-[var(--color-bg-card)] text-[var(--color-text-primary)] hover:bg-[var(--color-bg-tertiary)] disabled:bg-[var(--color-bg-card)]/50'
	);
</script>

<button
	{type}
	disabled={isDisabled}
	{...restProps}
	class="w-full px-4 py-3 font-medium rounded-lg transition-all duration-200
           relative overflow-hidden disabled:cursor-not-allowed
           {variantClasses}"
>
	<span class="relative z-10 flex items-center justify-center gap-2">
		{#if loading}
			<svg
				class="animate-spin h-5 w-5"
				xmlns="http://www.w3.org/2000/svg"
				fill="none"
				viewBox="0 0 24 24"
			>
				<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"
				></circle>
				<path
					class="opacity-75"
					fill="currentColor"
					d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
				></path>
			</svg>
		{/if}
		{@render children?.()}
	</span>

	<span
		class="absolute inset-0 bg-white/10 opacity-0 hover:opacity-100 transition-opacity duration-300"
	></span>
</button>
