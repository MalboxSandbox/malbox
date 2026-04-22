<script lang="ts">
	import type { PluginType } from '$lib/api/types';
	import type { PageData } from './$types';

	interface Props {
		data: PageData;
	}

	let { data }: Props = $props();

	let searchQuery = $state('');
	let activeFilter = $state<'all' | PluginType>('all');

	const filtered = $derived(() => {
		let items = data.plugins;
		if (activeFilter !== 'all') items = items.filter((p) => p.plugin_type === activeFilter);
		const q = searchQuery.trim().toLowerCase();
		if (q) {
			items = items.filter(
				(p) =>
					p.name.toLowerCase().includes(q) ||
					p.description?.toLowerCase().includes(q) ||
					p.version.toLowerCase().includes(q)
			);
		}
		return items;
	});
</script>

<div class="mx-auto max-w-7xl space-y-6">
	<div class="flex items-center gap-3 text-sm text-[var(--color-text-secondary)]">
		<a href="/settings" class="hover:text-[var(--color-text-primary)]">Settings</a>
		<span>/</span>
		<span class="text-[var(--color-text-primary)]">Installed plugins</span>
	</div>

	<div class="flex items-center justify-between gap-4">
		<h1 class="text-3xl font-semibold text-[var(--color-text-primary)]">Installed plugins</h1>

		<div class="flex items-center rounded-xl bg-[var(--color-bg-tertiary)] p-1.5">
			<button
				onclick={() => (activeFilter = 'all')}
				class="rounded-lg px-5 py-2.5 text-sm font-medium transition-colors {activeFilter === 'all'
					? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
			>
				All
			</button>
			<button
				onclick={() => (activeFilter = 'guest')}
				class="rounded-lg px-5 py-2.5 text-sm font-medium transition-colors {activeFilter ===
				'guest'
					? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
			>
				Guest
			</button>
			<button
				onclick={() => (activeFilter = 'host')}
				class="rounded-lg px-5 py-2.5 text-sm font-medium transition-colors {activeFilter === 'host'
					? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
			>
				Host
			</button>
		</div>
	</div>

	<div class="rounded-2xl bg-[var(--color-bg-secondary)] p-6 space-y-6">
		<div class="relative max-w-md">
			<input
				type="text"
				placeholder="Search plugins"
				bind:value={searchQuery}
				class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
			/>
		</div>

		{#if filtered().length === 0}
			<p class="text-center text-sm text-[var(--color-text-secondary)]">
				No plugins match the current filter.
			</p>
		{:else}
			<div class="overflow-hidden rounded-lg border border-[var(--color-border)]">
				<div
					class="grid grid-cols-[1fr_100px_100px_120px_120px_1fr] gap-4 border-b border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-4 py-2 text-xs font-medium text-[var(--color-text-secondary)]"
				>
					<div>Name</div>
					<div>Type</div>
					<div>Version</div>
					<div>Execution</div>
					<div>Status</div>
					<div>Description</div>
				</div>
				{#each filtered() as plugin (plugin.name)}
					<div
						class="grid grid-cols-[1fr_100px_100px_120px_120px_1fr] items-center gap-4 border-b border-[var(--color-border)] px-4 py-3 text-sm last:border-b-0"
					>
						<div class="text-[var(--color-text-primary)]">{plugin.name}</div>
						<div>
							<span
								class="rounded px-2 py-0.5 text-xs font-medium {plugin.plugin_type === 'guest'
									? 'bg-[#6B7FD9]/20 text-[#8B9AE8]'
									: 'bg-[#516CF9]/20 text-[#7B8FFF]'}"
							>
								{plugin.plugin_type}
							</span>
						</div>
						<div class="text-[var(--color-text-secondary)]">v{plugin.version}</div>
						<div class="text-[var(--color-text-secondary)]">{plugin.execution}</div>
						<div class="text-[var(--color-text-secondary)]">{plugin.status}</div>
						<div
							class="truncate text-[var(--color-text-secondary)]"
							title={plugin.description ?? ''}
						>
							{plugin.description ?? '—'}
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>
