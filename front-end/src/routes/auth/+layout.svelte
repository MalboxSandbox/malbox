<script lang="ts">
	import Logo from '$lib/components/ui/Logo.svelte';
</script>

<div
	class="min-h-screen bg-[var(--color-bg-primary)] flex items-center justify-center p-8 relative overflow-hidden"
>
	<!-- Decorative Elements -->
	<div
		class="absolute -right-40 -top-40 w-96 h-96 bg-[var(--color-accent)] opacity-10 rounded-full blur-3xl"
	></div>
	<div
		class="absolute -left-40 -bottom-40 w-96 h-96 bg-[var(--color-accent)] opacity-10 rounded-full blur-3xl"
	></div>

	<div class="w-full max-w-md flex flex-col gap-4">
		<div class="text-center">
			<div class="inline-block">
				<Logo />
			</div>
		</div>

		<div class="bg-[var(--color-bg-secondary)] border border-[var(--color-border)] rounded-2xl p-8">
			<slot />
		</div>

		<div class="text-center text-sm text-[var(--color-text-secondary)]">
			© 2025 Malbox. All rights reserved.
		</div>
	</div>
</div>
