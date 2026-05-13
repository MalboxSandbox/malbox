<script lang="ts">
	import type { MarketplaceItem } from '$lib/types/marketplace';

	interface Props {
		item: MarketplaceItem;
	}

	let { item }: Props = $props();

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

<a
	href="/marketplace/{item.id}"
	class="block bg-[var(--color-bg-secondary)] rounded-xl overflow-hidden hover:ring-2 hover:ring-[var(--color-accent)]/30 transition-all"
>
	<!-- Card Banner with Gradient -->
	<div class="relative h-44 {getGradientClass(item.avatarColor)} flex items-center justify-center">
		<!-- Avatar Circle -->
		<div
			class="w-28 h-28 rounded-full bg-white/20 backdrop-blur-sm flex items-center justify-center"
		>
			<span class="text-white text-4xl font-semibold">AP</span>
		</div>
	</div>

	<!-- Card Content -->
	<div class="p-6 space-y-3">
		<!-- Title and Badge -->
		<div class="flex items-center gap-2.5">
			<h3 class="text-[var(--color-text-primary)] text-lg font-semibold">{item.name}</h3>
			<span
				class="px-2.5 py-0.5 rounded text-xs font-medium {item.type === 'plugin'
					? 'bg-[#6B7FD9]/20 text-[#8B9AE8]'
					: 'bg-[#516CF9]/20 text-[#7B8FFF]'}"
			>
				{item.type === 'plugin' ? 'Plugin' : 'Module'}
			</span>
		</div>

		<!-- Author -->
		<p class="text-[var(--color-text-secondary)] text-sm">by {item.author}</p>

		<!-- Description -->
		<p class="text-[var(--color-text-secondary)] text-sm leading-relaxed">
			{item.description}
		</p>

		<!-- Rating -->
		<div class="flex items-center gap-0.5 pt-1">
			{#each renderStars(item.rating) as filled, i (i)}
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
</a>
