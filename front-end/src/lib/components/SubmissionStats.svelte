<script lang="ts">
	import type { TaskCounts } from '$lib/api/types';

	interface Props {
		counts: TaskCounts;
	}

	let { counts }: Props = $props();

	let statsTab = $state<'user' | 'globales'>('user');

	const completed = $derived(counts.by_status['completed'] ?? 0);
	const running = $derived(
		(counts.by_status['running'] ?? 0) +
			(counts.by_status['initializing'] ?? 0) +
			(counts.by_status['preparing_resources'] ?? 0) +
			(counts.by_status['stopping'] ?? 0)
	);
	const pending = $derived(counts.by_status['pending'] ?? 0);
</script>

<div class="rounded-2xl bg-[var(--color-bg-secondary)] p-6">
	<div class="mb-6 flex items-center justify-between border-b border-[var(--color-border)] pb-6">
		<h2 class="text-xl font-semibold text-[var(--color-text-primary)]">Submission Statistics</h2>
		<div class="flex gap-2">
			<button
				onclick={() => (statsTab = 'user')}
				class="rounded-lg px-3 py-1 text-sm transition-colors {statsTab === 'user'
					? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				>User</button
			>
			<button
				onclick={() => (statsTab = 'globales')}
				class="rounded-lg px-3 py-1 text-sm transition-colors {statsTab === 'globales'
					? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				>Globales</button
			>
		</div>
	</div>

	<div class="grid grid-cols-2 gap-4">
		<div class="flex flex-col gap-2 rounded-xl bg-[var(--color-bg-tertiary)] p-6">
			<div class="mb-2 text-sm text-[var(--color-text-secondary)]">Total submissions</div>
			<div class="text-5xl font-bold text-[var(--color-text-primary)]">{counts.total}</div>
		</div>
		<div class="flex flex-col gap-2 rounded-xl bg-[var(--color-bg-tertiary)] p-6">
			<div class="mb-2 text-sm text-[var(--color-text-secondary)]">Completed</div>
			<div class="text-5xl font-bold text-[var(--color-text-primary)]">{completed}</div>
		</div>
		<div class="flex flex-col gap-2 rounded-xl bg-[var(--color-bg-tertiary)] p-6">
			<div class="mb-2 text-sm text-[var(--color-text-secondary)]">Analysis in progress</div>
			<div class="text-5xl font-bold text-[var(--color-text-primary)]">{running}</div>
		</div>
		<div class="flex flex-col gap-2 rounded-xl bg-[var(--color-bg-tertiary)] p-6">
			<div class="mb-2 text-sm text-[var(--color-text-secondary)]">Pending analysis</div>
			<div class="text-5xl font-bold text-[var(--color-text-primary)]">{pending}</div>
		</div>
	</div>
</div>
