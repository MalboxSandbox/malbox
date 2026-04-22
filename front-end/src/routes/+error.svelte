<script lang="ts">
	import { page } from '$app/stores';

	const status = $derived($page.status);
	const message = $derived($page.error?.message ?? 'Something went wrong.');
</script>

<div class="flex min-h-screen items-center justify-center bg-[var(--color-bg-primary)] p-8">
	<div class="max-w-md space-y-4 text-center">
		<div class="text-5xl font-semibold text-[var(--color-text-primary)]">{status}</div>
		<p class="text-[var(--color-text-secondary)]">{message}</p>
		<a
			href="/dashboard"
			class="inline-block rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm text-white hover:bg-[var(--color-accent-hover)]"
		>
			Back to dashboard
		</a>
	</div>
</div>
