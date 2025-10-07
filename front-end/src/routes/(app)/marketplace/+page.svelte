<script lang="ts">
	import PluginCard from '$lib/components/PluginCard.svelte';
	import type { MarketplaceItem, MarketplaceFilter } from '$lib/types/marketplace';

	// State
	let searchQuery = $state('');
	let activeFilter = $state<MarketplaceFilter>('all');

	// Mock data
	const officialItems: MarketplaceItem[] = [
		{
			id: '1',
			name: 'Another Plugin',
			type: 'plugin',
			author: 'ProductName',
			description:
				'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore...',
			rating: 4,
			avatarColor: 'blue',
			isOfficial: true
		},
		{
			id: '2',
			name: 'Another Plugin',
			type: 'module',
			author: 'ProductName',
			description:
				'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore...',
			rating: 5,
			avatarColor: 'red',
			isOfficial: true
		},
		{
			id: '3',
			name: 'Another Plugin',
			type: 'plugin',
			author: 'ProductName',
			description:
				'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore...',
			rating: 3,
			avatarColor: 'blue',
			isOfficial: true
		}
	];

	const communityItems: MarketplaceItem[] = [
		{
			id: '4',
			name: 'Community Plugin',
			type: 'plugin',
			author: 'CommunityDev',
			description:
				'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore...',
			rating: 4,
			avatarColor: 'red',
			isOfficial: false
		},
		{
			id: '5',
			name: 'Community Module',
			type: 'module',
			author: 'CommunityDev',
			description:
				'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore...',
			rating: 4,
			avatarColor: 'blue',
			isOfficial: false
		},
		{
			id: '6',
			name: 'Community Plugin',
			type: 'plugin',
			author: 'CommunityDev',
			description:
				'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore...',
			rating: 5,
			avatarColor: 'red',
			isOfficial: false
		}
	];

	// Filtered items
	const filteredOfficial = $derived(() => {
		let filtered = officialItems;
		if (activeFilter === 'modules') filtered = filtered.filter((i) => i.type === 'module');
		if (activeFilter === 'plugins') filtered = filtered.filter((i) => i.type === 'plugin');
		if (searchQuery.trim()) {
			const query = searchQuery.toLowerCase();
			filtered = filtered.filter(
				(i) =>
					i.name.toLowerCase().includes(query) ||
					i.author.toLowerCase().includes(query) ||
					i.description.toLowerCase().includes(query)
			);
		}
		return filtered;
	});

	const filteredCommunity = $derived(() => {
		let filtered = communityItems;
		if (activeFilter === 'modules') filtered = filtered.filter((i) => i.type === 'module');
		if (activeFilter === 'plugins') filtered = filtered.filter((i) => i.type === 'plugin');
		if (searchQuery.trim()) {
			const query = searchQuery.toLowerCase();
			filtered = filtered.filter(
				(i) =>
					i.name.toLowerCase().includes(query) ||
					i.author.toLowerCase().includes(query) ||
					i.description.toLowerCase().includes(query)
			);
		}
		return filtered;
	});
</script>

<div class="max-w-7xl mx-auto space-y-8">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<h1 class="text-[var(--color-text-primary)] text-3xl font-semibold">Marketplace</h1>

		<div class="flex items-center gap-3">
			<!-- Filter Tabs -->
			<div class="flex items-center bg-[var(--color-bg-tertiary)] rounded-xl p-1.5">
				<button
					onclick={() => (activeFilter = 'all')}
					class="px-5 py-2.5 rounded-lg text-sm font-medium transition-colors {activeFilter ===
					'all'
						? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
						: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				>
					All
				</button>
				<button
					onclick={() => (activeFilter = 'modules')}
					class="px-5 py-2.5 rounded-lg text-sm font-medium transition-colors {activeFilter ===
					'modules'
						? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
						: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				>
					Modules
				</button>
				<button
					onclick={() => (activeFilter = 'plugins')}
					class="px-5 py-2.5 rounded-lg text-sm font-medium transition-colors {activeFilter ===
					'plugins'
						? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
						: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				>
					Plugins
				</button>
			</div>

			<!-- Filters Button -->
			<button
				class="px-6 py-3.5 bg-[var(--color-accent)] hover:bg-[var(--color-accent-hover)] text-[var(--color-text-primary)] text-sm font-medium rounded-xl transition-colors"
			>
				Filters
			</button>
		</div>
	</div>

	<!-- Hero Section -->
	<div class="bg-[var(--color-bg-card)] rounded-2xl px-16 py-12 space-y-6">
		<!-- Hero Text -->
		<div class="text-center space-y-4">
			<h2 class="text-[var(--color-text-primary)] text-3xl font-semibold">
				Discover a wide range of modules and plugins to suit all requirements
			</h2>
			<p class="text-[var(--color-text-secondary)] text-sm">
				Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt
				ut labore et dolore magna aliqua. Ut enim ad minim veniam.
			</p>
		</div>

		<!-- Search Bar -->
		<div class="relative max-w-3xl mx-auto">
			<svg
				class="absolute left-4 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--color-text-secondary)]"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
			>
				<circle cx="11" cy="11" r="8" />
				<path d="m21 21-4.35-4.35" />
			</svg>
			<input
				type="text"
				placeholder="Search"
				bind:value={searchQuery}
				class="w-full pl-12 pr-4 py-3 bg-[var(--color-bg-tertiary)] rounded-lg text-[var(--color-text-primary)] text-sm placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30 transition-all"
			/>
		</div>
	</div>

	<!-- Official Section -->
	<div class="space-y-4">
		<div>
			<h2 class="text-[var(--color-text-primary)] text-xl font-semibold">Official</h2>
			<p class="text-[var(--color-text-secondary)] text-sm">
				Official modules and plugins created by the team
			</p>
		</div>

		<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
			{#each filteredOfficial() as item}
				<PluginCard {item} />
			{/each}
		</div>
	</div>

	<!-- Community Section -->
	<div class="space-y-4">
		<div>
			<h2 class="text-[var(--color-text-primary)] text-xl font-semibold">Community</h2>
			<p class="text-[var(--color-text-secondary)] text-sm">
				Modules and plugins created by the community
			</p>
		</div>

		<div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
			{#each filteredCommunity() as item}
				<PluginCard {item} />
			{/each}
		</div>
	</div>
</div>
