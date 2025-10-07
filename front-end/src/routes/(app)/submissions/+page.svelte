<script lang="ts">
	import SubmissionsTable from '$lib/components/SubmissionsTable.svelte';
	import FiltersModal from '$lib/components/FiltersModal.svelte';
	import type { SubmissionItem, SubmissionFilter } from '$lib/types/submission';

	// State
	let searchQuery = $state('');
	let activeFilter = $state<SubmissionFilter>('all');
	let showFilters = $state(false);

	// Mock data
	const allSubmissions: SubmissionItem[] = [
		{
			id: '1',
			date: '2024-04-06',
			time: '14:45',
			type: 'url',
			value: 'google.com/share',
			status: 'finished',
			progress: 3,
			total: 10,
			apiKey: null
		},
		{
			id: '2',
			date: '2024-04-06',
			time: '14:45',
			type: 'url',
			value: 'google.com/share',
			status: 'finished',
			progress: 3,
			total: 10,
			apiKey: 'Personnel Project'
		},
		{
			id: '3',
			date: '2024-04-06',
			time: '14:45',
			type: 'file',
			value: 'malware_sample.exe',
			status: 'finished',
			progress: 3,
			total: 10,
			apiKey: null
		},
		{
			id: '4',
			date: '2024-04-06',
			time: '14:45',
			type: 'hash',
			value: 'a3f5e8d2c1b4f6e9a2d5c8b7f4e1d3c6',
			status: 'pending',
			progress: 0,
			total: 10,
			apiKey: 'Personnel Project'
		},
		{
			id: '5',
			date: '2024-04-06',
			time: '14:45',
			type: 'url',
			value: 'google.com/share',
			status: 'pending',
			progress: 0,
			total: 10,
			apiKey: null
		},
		{
			id: '6',
			date: '2024-04-06',
			time: '14:45',
			type: 'file',
			value: 'suspicious_document.pdf',
			status: 'pending',
			progress: 0,
			total: 10,
			apiKey: null
		},
		{
			id: '7',
			date: '2024-04-06',
			time: '14:45',
			type: 'url',
			value: 'google.com/share',
			status: 'pending',
			progress: 0,
			total: 10,
			apiKey: 'Personnel Project'
		}
	];

	// Filtered submissions
	const filteredSubmissions = $derived(() => {
		let filtered = allSubmissions;

		// Apply status filter
		if (activeFilter !== 'all') {
			filtered = filtered.filter((s) => s.status === activeFilter);
		}

		// Apply search filter
		if (searchQuery.trim()) {
			const query = searchQuery.toLowerCase();
			filtered = filtered.filter(
				(s) =>
					s.value.toLowerCase().includes(query) ||
					s.apiKey?.toLowerCase().includes(query) ||
					s.date.includes(query)
			);
		}

		return filtered;
	});

	function handleApplyFilters(filters: any) {
		console.log('Applied filters:', filters);
		// TODO: Apply advanced filters to submissions
	}
</script>

<div class="max-w-7xl mx-auto space-y-6">
	<!-- Search and Filters -->
	<div class="flex items-center justify-between gap-4">
		<!-- Search Bar -->
		<div class="flex-1 max-w-md relative">
			<input
				type="text"
				placeholder="Search"
				bind:value={searchQuery}
				class="w-full px-4 py-4 bg-[var(--color-bg-tertiary)] rounded-xl text-[var(--color-text-primary)] text-sm placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:border-[var(--color-accent)] transition-colors"
			/>
			<svg
				class="absolute right-4 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--color-text-secondary)]"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
			>
				<circle cx="11" cy="11" r="8" />
				<path d="m21 21-4.35-4.35" />
			</svg>
		</div>

		<!-- Filter Buttons -->
		<div class="flex items-center gap-6">
			<div class="flex items-center bg-[var(--color-bg-tertiary)] rounded-xl p-1.5">
				<button
					onclick={() => (activeFilter = 'all')}
					class="px-4 py-2.5 rounded-lg text-sm font-medium transition-colors {activeFilter ===
					'all'
						? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
						: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				>
					All
				</button>
				<button
					onclick={() => (activeFilter = 'finished')}
					class="px-4 py-1.5 rounded text-sm font-medium transition-colors {activeFilter ===
					'finished'
						? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
						: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				>
					Finished
				</button>
				<button
					onclick={() => (activeFilter = 'pending')}
					class="px-4 py-1.5 rounded text-sm font-medium transition-colors {activeFilter ===
					'pending'
						? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
						: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				>
					Pending
				</button>
			</div>

			<button
				onclick={() => (showFilters = true)}
				class="px-6 py-3.5 bg-[var(--color-accent)] hover:bg-[var(--color-accent-hover)] text-[var(--color-text-primary)] text-sm font-medium rounded-lg transition-colors"
			>
				Filters
			</button>
		</div>
	</div>

	<!-- Submissions Table -->
	<SubmissionsTable submissions={filteredSubmissions()} />
</div>

<!-- Filters Modal -->
<FiltersModal
	isOpen={showFilters}
	onClose={() => (showFilters = false)}
	onApply={handleApplyFilters}
/>
