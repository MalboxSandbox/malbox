<script lang="ts">
	import SubmissionsTable from '$lib/components/SubmissionsTable.svelte';
	import FiltersModal from '$lib/components/FiltersModal.svelte';
	import type { PageData } from './$types';
	import type { Task } from '$lib/api/types';

	interface Props {
		data: PageData;
	}

	let { data }: Props = $props();

	let searchQuery = $state('');
	let activeFilter = $state<'all' | 'finished' | 'pending'>('all');
	let showFilters = $state(false);

	function matchesFilter(task: Task, filter: typeof activeFilter): boolean {
		if (filter === 'all') return true;
		if (filter === 'finished') return task.status === 'completed';
		if (filter === 'pending') return task.status === 'pending' || task.status === 'running';
		return true;
	}

	const filtered = $derived(() => {
		let list = data.tasks;
		list = list.filter((t) => matchesFilter(t, activeFilter));
		const q = searchQuery.trim().toLowerCase();
		if (q) {
			list = list.filter(
				(t) =>
					t.target.toLowerCase().includes(q) ||
					t.tags?.some((tag) => tag.toLowerCase().includes(q)) ||
					t.created_on.includes(q)
			);
		}
		return list;
	});
</script>

<div class="mx-auto max-w-7xl space-y-6">
	<div class="flex items-center justify-between gap-4">
		<div class="relative max-w-md flex-1">
			<input
				type="text"
				placeholder="Search"
				bind:value={searchQuery}
				class="w-full rounded-xl bg-[var(--color-bg-tertiary)] px-4 py-4 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] transition-colors focus:border-[var(--color-accent)] focus:outline-none"
			/>
		</div>

		<div class="flex items-center gap-6">
			<div class="flex items-center rounded-xl bg-[var(--color-bg-tertiary)] p-1.5">
				<button
					onclick={() => (activeFilter = 'all')}
					class="rounded-lg px-4 py-2.5 text-sm font-medium transition-colors {activeFilter ===
					'all'
						? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
						: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				>
					All
				</button>
				<button
					onclick={() => (activeFilter = 'finished')}
					class="rounded px-4 py-1.5 text-sm font-medium transition-colors {activeFilter ===
					'finished'
						? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
						: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				>
					Finished
				</button>
				<button
					onclick={() => (activeFilter = 'pending')}
					class="rounded px-4 py-1.5 text-sm font-medium transition-colors {activeFilter ===
					'pending'
						? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
						: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				>
					Pending
				</button>
			</div>

			<button
				onclick={() => (showFilters = true)}
				class="rounded-lg bg-[var(--color-accent)] px-6 py-3.5 text-sm font-medium text-[var(--color-text-primary)] transition-colors hover:bg-[var(--color-accent-hover)]"
			>
				Filters
			</button>
		</div>
	</div>

	<SubmissionsTable tasks={filtered()} />
</div>

<FiltersModal isOpen={showFilters} onClose={() => (showFilters = false)} onApply={() => {}} />
