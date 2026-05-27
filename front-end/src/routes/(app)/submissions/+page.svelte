<script lang="ts">
	import SubmissionsTable from '$lib/components/SubmissionsTable.svelte';
	import FiltersModal from '$lib/components/FiltersModal.svelte';
	import { goto } from '$app/navigation';
	import { listTasks } from '$lib/api/tasks';
	import type { PageData } from './$types';
	import type { Task } from '$lib/api/types';

	interface Props {
		data: PageData;
	}

	let { data }: Props = $props();

	let searchQuery = $state('');
	let showFilters = $state(false);
	let loadingMore = $state(false);
	let extraTasks = $state<Task[]>([]);
	let nextCursor = $state<string | null>(null);
	let hasMore = $state(false);

	$effect(() => {
		extraTasks = [];
		nextCursor = data.tasks.next_cursor;
		hasMore = data.tasks.has_more;
	});

	const activeFilter = $derived(data.filter as 'all' | 'finished' | 'pending');

	function setFilter(filter: 'all' | 'finished' | 'pending') {
		const qs = filter !== 'all' ? `?filter=${filter}` : '';
		goto(`/submissions${qs}`, {
			keepFocus: true,
			noScroll: true
		});
	}

	function filterToStatus(filter: string): string | undefined {
		switch (filter) {
			case 'finished':
				return 'completed';
			case 'pending':
				return 'pending,running,initializing,preparing_resources,stopping';
			default:
				return undefined;
		}
	}

	async function loadMore() {
		if (loadingMore || !nextCursor) return;
		loadingMore = true;
		try {
			const page = await listTasks(globalThis.fetch, {
				status: filterToStatus(activeFilter),
				cursor: nextCursor,
				limit: 50
			});
			extraTasks = [...extraTasks, ...page.items];
			nextCursor = page.next_cursor;
			hasMore = page.has_more;
		} finally {
			loadingMore = false;
		}
	}

	const allTasks = $derived([...data.tasks.items, ...extraTasks]);

	const filtered = $derived(() => {
		const q = searchQuery.trim().toLowerCase();
		if (!q) return allTasks;
		return allTasks.filter(
			(t) =>
				t.target.toLowerCase().includes(q) ||
				t.tags?.some((tag) => tag.toLowerCase().includes(q)) ||
				t.created_on.includes(q)
		);
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
					onclick={() => setFilter('all')}
					class="rounded-lg px-4 py-2.5 text-sm font-medium transition-colors {activeFilter ===
					'all'
						? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
						: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				>
					All
				</button>
				<button
					onclick={() => setFilter('finished')}
					class="rounded px-4 py-1.5 text-sm font-medium transition-colors {activeFilter ===
					'finished'
						? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
						: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				>
					Finished
				</button>
				<button
					onclick={() => setFilter('pending')}
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

	{#if hasMore}
		<div class="flex justify-center">
			<button
				onclick={loadMore}
				disabled={loadingMore}
				class="rounded-xl bg-[var(--color-bg-secondary)] px-8 py-3 text-sm font-medium text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-primary)] disabled:cursor-not-allowed disabled:opacity-50"
			>
				{#if loadingMore}
					Loading...
				{:else}
					Load More
				{/if}
			</button>
		</div>
	{/if}
</div>

<FiltersModal isOpen={showFilters} onClose={() => (showFilters = false)} onApply={() => {}} />
