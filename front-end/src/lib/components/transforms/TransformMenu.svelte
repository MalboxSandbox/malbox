<script lang="ts">
	import { transformStore } from '$lib/stores/transforms.svelte';
	import type { TransformDefinition } from '$lib/transforms';

	interface Props {
		onSelect: (transform: TransformDefinition) => void;
	}

	let { onSelect }: Props = $props();
	let open = $state(false);
	let search = $state('');
	let menuRef = $state<HTMLDivElement | null>(null);

	const filtered = $derived.by(() => {
		const all = transformStore.transforms;
		if (!search.trim()) return all;
		const q = search.toLowerCase();
		return all.filter(
			(t) =>
				t.name.toLowerCase().includes(q) || t.id.toLowerCase().includes(q) || t.category.includes(q)
		);
	});

	const grouped = $derived.by(() => {
		const groups: Record<string, TransformDefinition[]> = {};
		for (const t of filtered) {
			(groups[t.category] ??= []).push(t);
		}
		return groups;
	});

	function select(t: TransformDefinition) {
		onSelect(t);
		close();
	}

	function close() {
		open = false;
		search = '';
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') close();
	}

	function handleClickOutside(e: MouseEvent) {
		if (menuRef && !menuRef.contains(e.target as Node)) {
			close();
		}
	}

	$effect(() => {
		if (open) {
			document.addEventListener('click', handleClickOutside, true);
			document.addEventListener('keydown', handleKeydown);
			return () => {
				document.removeEventListener('click', handleClickOutside, true);
				document.removeEventListener('keydown', handleKeydown);
			};
		}
	});
</script>

<div class="relative" bind:this={menuRef}>
	<button
		onclick={() => (open = !open)}
		class="rounded p-1.5 text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-primary)]"
		title="Apply transform"
	>
		<svg
			xmlns="http://www.w3.org/2000/svg"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			class="h-4 w-4"
		>
			<path
				d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"
			/>
		</svg>
	</button>

	{#if open}
		<div
			class="absolute right-0 z-50 mt-1 w-72 rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-secondary)] shadow-lg"
		>
			<div class="p-2">
				<input
					type="text"
					placeholder="Search transforms..."
					bind:value={search}
					class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none"
				/>
			</div>
			<div class="max-h-64 overflow-y-auto px-2 pb-2">
				{#each Object.entries(grouped) as [category, transforms] (category)}
					<div class="mb-2">
						<div
							class="px-2 py-1 text-[10px] font-semibold uppercase tracking-wider text-[var(--color-text-secondary)]"
						>
							{category}
						</div>
						{#each transforms as t (t.id)}
							<button
								onclick={() => select(t)}
								class="w-full rounded-lg px-3 py-1.5 text-left text-sm text-[var(--color-text-primary)] transition-colors hover:bg-[var(--color-bg-tertiary)]"
							>
								{t.name}
								{#if t.provenance !== 'builtin'}
									<span class="ml-1 text-[10px] text-[var(--color-text-secondary)]"
										>({t.provenance})</span
									>
								{/if}
							</button>
						{/each}
					</div>
				{/each}
				{#if Object.keys(grouped).length === 0}
					<p class="px-3 py-2 text-sm text-[var(--color-text-secondary)]">No transforms found</p>
				{/if}
			</div>
		</div>
	{/if}
</div>
