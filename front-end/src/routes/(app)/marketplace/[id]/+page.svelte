<script lang="ts">
	import { page } from '$app/stores';

	// Mock data - in production this would come from a load function
	const plugin = {
		id: $page.params.id,
		name: 'Another Plugin',
		type: 'plugin' as const,
		author: 'ProductName',
		downloads: 1844,
		fileSize: '10 Mo',
		rating: 4,
		avatarColor: 'blue' as const,
		description: `Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.`
	};

	let activeTab = $state<'description' | 'demo' | 'changelog'>('description');

	function getGradientClass(color: 'blue' | 'red') {
		return color === 'blue'
			? 'bg-gradient-to-br from-[#7B8FD6] via-[#6275BE] to-[#4A5B9D]'
			: 'bg-gradient-to-br from-[#D86B7D] via-[#C4586A] to-[#9D4656]';
	}

	function renderStars(rating: number) {
		const stars = [];
		for (let i = 1; i <= 5; i++) {
			stars.push(i <= rating);
		}
		return stars;
	}
</script>

<div class="max-w-7xl mx-auto space-y-8">
	<!-- Breadcrumb -->
	<div class="text-[var(--color-text-secondary)] text-sm">
		<a href="/marketplace" class="hover:text-[var(--color-text-primary)] transition-colors">
			Marketplace
		</a>
		<span class="mx-2">/</span>
		<span class="text-[var(--color-text-primary)]">{plugin.name}</span>
	</div>

	<!-- Hero Banner Card -->
	<div class="bg-[var(--color-bg-secondary)] rounded-2xl overflow-hidden">
		<!-- Gradient Banner -->
		<div class="relative h-48 {getGradientClass(plugin.avatarColor)}"></div>

		<!-- Content Section -->
		<div class="relative px-8 pb-8">
			<!-- Avatar positioned to overlap banner -->
			<div
				class="absolute -top-16 left-8 w-32 h-32 rounded-2xl bg-white/20 backdrop-blur-sm flex items-center justify-center"
			>
				<span class="text-white text-5xl font-semibold">AP</span>
			</div>

			<!-- Top Section with Title and Buttons -->
			<div class="pt-20 flex items-start justify-between">
				<div class="space-y-2">
					<div class="flex items-center gap-3">
						<h1 class="text-[var(--color-text-primary)] text-3xl font-semibold">
							{plugin.name}
						</h1>
						<span
							class="px-3 py-1 rounded-md text-sm font-medium {plugin.type === 'plugin'
								? 'bg-[#6B7FD9]/20 text-[#8B9AE8]'
								: 'bg-[#516CF9]/20 text-[#7B8FFF]'}"
						>
							{plugin.type === 'plugin' ? 'Plugin' : 'Module'}
						</span>
					</div>
					<p class="text-[var(--color-text-secondary)] text-sm">by {plugin.author}</p>
				</div>

				<div class="flex items-center gap-3">
					<button
						class="px-6 py-3 bg-[var(--color-bg-tertiary)] hover:bg-[var(--color-tab-active)] text-[var(--color-text-primary)] text-sm font-medium rounded-lg transition-colors"
					>
						Source code
					</button>
					<button
						class="px-6 py-3 bg-[var(--color-accent)] hover:bg-[var(--color-accent-hover)] text-[var(--color-text-primary)] text-sm font-medium rounded-lg transition-colors"
					>
						Download {plugin.name}
					</button>
				</div>
			</div>

			<!-- Stats Section -->
			<div class="mt-8 flex items-center gap-12">
				<div>
					<div class="text-[var(--color-text-secondary)] text-sm mb-1">Number of downloads</div>
					<div class="text-[var(--color-text-primary)] text-base font-medium">
						{plugin.downloads} Downloads
					</div>
				</div>

				<div>
					<div class="text-[var(--color-text-secondary)] text-sm mb-1">File size</div>
					<div class="text-[var(--color-text-primary)] text-base font-medium">
						{plugin.fileSize}
					</div>
				</div>

				<div>
					<div class="text-[var(--color-text-secondary)] text-sm mb-1">Review</div>
					<div class="flex items-center gap-0.5">
						{#each renderStars(plugin.rating) as filled}
							<svg
								class="w-5 h-5 {filled ? 'text-[#FDB022]' : 'text-[#3D3F47]'}"
								fill="currentColor"
								viewBox="0 0 20 20"
							>
								<path
									d="M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z"
								/>
							</svg>
						{/each}
					</div>
				</div>
			</div>
		</div>
	</div>

	<!-- Tabs and Content -->
	<div class="space-y-8">
		<!-- Tab Navigation -->
		<div class="flex items-center gap-8 border-b border-[var(--color-border)]">
			<button
				onclick={() => (activeTab = 'description')}
				class="pb-3 text-sm font-medium transition-colors relative {activeTab === 'description'
					? 'text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
			>
				Description
				{#if activeTab === 'description'}
					<div class="absolute bottom-0 left-0 right-0 h-0.5 bg-[var(--color-text-primary)]"></div>
				{/if}
			</button>
			<button
				onclick={() => (activeTab = 'demo')}
				class="pb-3 text-sm font-medium transition-colors relative {activeTab === 'demo'
					? 'text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
			>
				Demo
				{#if activeTab === 'demo'}
					<div class="absolute bottom-0 left-0 right-0 h-0.5 bg-[var(--color-text-primary)]"></div>
				{/if}
			</button>
			<button
				onclick={() => (activeTab = 'changelog')}
				class="pb-3 text-sm font-medium transition-colors relative {activeTab === 'changelog'
					? 'text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
			>
				Changelog
				{#if activeTab === 'changelog'}
					<div class="absolute bottom-0 left-0 right-0 h-0.5 bg-[var(--color-text-primary)]"></div>
				{/if}
			</button>
		</div>

		<!-- Tab Content -->
		{#if activeTab === 'description'}
			<div class="space-y-8">
				<!-- Section 1 -->
				<div class="space-y-4">
					<h2 class="text-[var(--color-text-primary)] text-xl font-semibold">
						Submission Rules and Criteria
					</h2>
					<p class="text-[var(--color-text-secondary)] text-sm leading-relaxed">
						Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor
						incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud
						exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.
					</p>
				</div>

				<!-- Section 2 -->
				<div class="space-y-4">
					<h2 class="text-[var(--color-text-primary)] text-xl font-semibold">
						Code of Conduct, setting forth the expected behavior
					</h2>
					<p class="text-[var(--color-text-secondary)] text-sm leading-relaxed">
						Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do.
					</p>
				</div>

				<!-- Section 3 -->
				<div class="space-y-4">
					<h3 class="text-[var(--color-text-primary)] text-lg font-semibold">
						1. Disqualification Criteria
					</h3>
					<p class="text-[var(--color-text-secondary)] text-sm leading-relaxed">
						Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor
						incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud
						exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.
					</p>
				</div>

				<!-- Section 4 -->
				<div class="space-y-4">
					<h3 class="text-[var(--color-text-primary)] text-lg font-semibold">
						2. Winners Announcement
					</h3>
					<p class="text-[var(--color-text-secondary)] text-sm leading-relaxed">
						Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor
						incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud
						exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.Lorem ipsum dolor
						sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et
						dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris
						nisi ut aliquip ex ea commodo consequat. Lorem ipsum dolor sit amet, consectetur
						adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut
						enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea
						commodo consequat.
					</p>
				</div>

				<!-- Section 5 -->
				<div class="space-y-4">
					<h3 class="text-[var(--color-text-primary)] text-lg font-semibold">
						3. Deadline Details
					</h3>
					<p class="text-[var(--color-text-secondary)] text-sm leading-relaxed">
						Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor
						incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud
						exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.
					</p>
				</div>

				<!-- Section 6 -->
				<div class="space-y-4">
					<h3 class="text-[var(--color-text-primary)] text-lg font-semibold">
						4. Entry Requirements
					</h3>
				</div>
			</div>
		{:else if activeTab === 'demo'}
			<div class="text-[var(--color-text-secondary)] text-sm">Demo content coming soon...</div>
		{:else}
			<div class="text-[var(--color-text-secondary)] text-sm">Changelog content coming soon...</div>
		{/if}
	</div>
</div>
