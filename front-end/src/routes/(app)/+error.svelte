<script lang="ts">
	import { page } from '$app/stores';

	const status = $derived($page.status);
	const message = $derived($page.error?.message ?? 'Something went wrong.');

	const headline = $derived.by(() => {
		if (status === 404) return 'Not found';
		if (status === 502) return 'Back-end unreachable';
		if (status >= 500) return 'Server error';
		return 'Error';
	});
</script>

<div
	class="mx-auto max-w-3xl space-y-4 rounded-2xl bg-[var(--color-bg-secondary)] p-10 text-center"
>
	<div class="text-sm font-medium uppercase tracking-wider text-[var(--color-text-secondary)]">
		{status}
	</div>
	<h1 class="text-2xl font-semibold text-[var(--color-text-primary)]">{headline}</h1>
	<p class="text-[var(--color-text-secondary)]">{message}</p>
</div>
