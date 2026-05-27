<script lang="ts">
	import PlatformLabel from '$lib/components/ui/PlatformLabel.svelte';
	import type { MachineStatus } from '$lib/api/types';
	import type { PageData } from './$types';

	interface Props {
		data: PageData;
	}

	let { data }: Props = $props();

	let searchQuery = $state('');
	let activeFilter = $state<'all' | MachineStatus>('all');

	const statusConfig: Record<MachineStatus, { label: string; dot: string; badge: string }> = {
		ready: {
			label: 'Ready',
			dot: 'bg-emerald-400',
			badge: 'bg-emerald-400/10 text-emerald-400'
		},
		assigned: {
			label: 'Assigned',
			dot: 'bg-[var(--color-accent)]',
			badge: 'bg-[var(--color-accent)]/10 text-[var(--color-accent)]'
		},
		creating: {
			label: 'Creating',
			dot: 'bg-amber-400',
			badge: 'bg-amber-400/10 text-amber-400'
		},
		provisioning: {
			label: 'Provisioning',
			dot: 'bg-amber-400',
			badge: 'bg-amber-400/10 text-amber-400'
		},
		reverting: {
			label: 'Reverting',
			dot: 'bg-amber-400',
			badge: 'bg-amber-400/10 text-amber-400'
		},
		deleting: {
			label: 'Deleting',
			dot: 'bg-orange-400',
			badge: 'bg-orange-400/10 text-orange-400'
		},
		failed: {
			label: 'Failed',
			dot: 'bg-red-400',
			badge: 'bg-red-400/10 text-red-400'
		}
	};

	const filterTabs: { value: 'all' | MachineStatus; label: string }[] = [
		{ value: 'all', label: 'All' },
		{ value: 'ready', label: 'Ready' },
		{ value: 'provisioning', label: 'Provisioning' },
		{ value: 'failed', label: 'Failed' }
	];

	const filtered = $derived(() => {
		let items = data.machines;
		if (activeFilter !== 'all') items = items.filter((m) => m.status === activeFilter);
		const q = searchQuery.trim().toLowerCase();
		if (q) {
			items = items.filter(
				(m) =>
					m.name.toLowerCase().includes(q) ||
					m.ip?.toLowerCase().includes(q) ||
					m.platform.toLowerCase().includes(q) ||
					m.provider?.toLowerCase().includes(q) ||
					m.image_name?.toLowerCase().includes(q)
			);
		}
		return items;
	});

	const counts = $derived(() => {
		const c: Record<string, number> = { total: data.machines.length };
		for (const m of data.machines) {
			c[m.status] = (c[m.status] ?? 0) + 1;
		}
		return c;
	});
</script>

<div class="mx-auto max-w-7xl space-y-6">
	<div class="flex items-center gap-3 text-sm text-[var(--color-text-secondary)]">
		<a href="/settings" class="hover:text-[var(--color-text-primary)]">Settings</a>
		<span>/</span>
		<span class="text-[var(--color-text-primary)]">Machines</span>
	</div>

	<div class="flex items-center justify-between gap-4">
		<h1 class="text-3xl font-semibold text-[var(--color-text-primary)]">Machines</h1>

		<div class="flex items-center rounded-xl bg-[var(--color-bg-tertiary)] p-1.5">
			{#each filterTabs as tab (tab.value)}
				<button
					onclick={() => (activeFilter = tab.value)}
					class="rounded-lg px-5 py-2.5 text-sm font-medium transition-colors {activeFilter ===
					tab.value
						? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
						: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				>
					{tab.label}
					{#if tab.value !== 'all' && counts()[tab.value]}
						<span class="ml-1 text-xs opacity-60">{counts()[tab.value]}</span>
					{/if}
				</button>
			{/each}
		</div>
	</div>

	{#if data.machines.length > 0}
		<div class="grid grid-cols-2 gap-4 md:grid-cols-4">
			<div class="rounded-xl bg-[var(--color-bg-secondary)] p-4">
				<div class="text-xs font-medium text-[var(--color-text-secondary)]">Total</div>
				<div class="mt-1 text-2xl font-semibold text-[var(--color-text-primary)]">
					{counts().total}
				</div>
			</div>
			<div class="rounded-xl bg-[var(--color-bg-secondary)] p-4">
				<div class="flex items-center gap-2 text-xs font-medium text-[var(--color-text-secondary)]">
					<span class="inline-block h-2 w-2 rounded-full bg-emerald-400"></span>
					Ready
				</div>
				<div class="mt-1 text-2xl font-semibold text-emerald-400">
					{counts().ready ?? 0}
				</div>
			</div>
			<div class="rounded-xl bg-[var(--color-bg-secondary)] p-4">
				<div class="flex items-center gap-2 text-xs font-medium text-[var(--color-text-secondary)]">
					<span class="inline-block h-2 w-2 rounded-full bg-[var(--color-accent)]"></span>
					Assigned
				</div>
				<div class="mt-1 text-2xl font-semibold text-[var(--color-accent)]">
					{counts().assigned ?? 0}
				</div>
			</div>
			<div class="rounded-xl bg-[var(--color-bg-secondary)] p-4">
				<div class="flex items-center gap-2 text-xs font-medium text-[var(--color-text-secondary)]">
					<span class="inline-block h-2 w-2 rounded-full bg-red-400"></span>
					Failed
				</div>
				<div class="mt-1 text-2xl font-semibold text-red-400">
					{counts().failed ?? 0}
				</div>
			</div>
		</div>
	{/if}

	<div class="rounded-2xl bg-[var(--color-bg-secondary)] p-6 space-y-6">
		<div class="relative max-w-md">
			<input
				type="text"
				placeholder="Search machines"
				bind:value={searchQuery}
				class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
			/>
		</div>

		{#if filtered().length === 0}
			<div class="py-12 text-center">
				<div
					class="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-xl bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)]"
				>
					<svg
						xmlns="http://www.w3.org/2000/svg"
						viewBox="0 0 20 20"
						fill="currentColor"
						class="size-6"
					>
						<path
							fill-rule="evenodd"
							d="M1 5.25A2.25 2.25 0 0 1 3.25 3h13.5A2.25 2.25 0 0 1 19 5.25v1.5A2.25 2.25 0 0 1 16.75 9H3.25A2.25 2.25 0 0 1 1 6.75v-1.5ZM14 6a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5ZM16.5 6a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5Z"
							clip-rule="evenodd"
						/>
						<path
							fill-rule="evenodd"
							d="M1 13.25A2.25 2.25 0 0 1 3.25 11h13.5A2.25 2.25 0 0 1 19 13.25v1.5A2.25 2.25 0 0 1 16.75 17H3.25A2.25 2.25 0 0 1 1 14.75v-1.5ZM14 14a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5ZM16.5 14a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5Z"
							clip-rule="evenodd"
						/>
					</svg>
				</div>
				<p class="text-sm text-[var(--color-text-secondary)]">
					{data.machines.length === 0
						? 'No machines registered.'
						: 'No machines match the current filter.'}
				</p>
			</div>
		{:else}
			<div class="overflow-hidden rounded-lg border border-[var(--color-border)]">
				<div
					class="grid grid-cols-[60px_1fr_120px_100px_160px_100px] gap-4 border-b border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-4 py-2 text-xs font-medium text-[var(--color-text-secondary)]"
				>
					<div>ID</div>
					<div>Name</div>
					<div>Platform</div>
					<div>Status</div>
					<div>IP</div>
					<div>Provider</div>
				</div>

				{#each filtered() as m (m.id ?? m.name)}
					<a
						href={m.id != null ? `/settings/machines/${m.id}` : '#'}
						class="grid grid-cols-[60px_1fr_120px_100px_160px_100px] items-center gap-4 border-b border-[var(--color-border)] px-4 py-3 text-sm transition-colors last:border-b-0 hover:bg-[var(--color-bg-tertiary)]"
					>
						<div class="text-[var(--color-text-secondary)] font-mono text-xs">
							{m.id != null ? `#${m.id}` : '-'}
						</div>
						<div>
							<div class="text-[var(--color-text-primary)]">{m.name}</div>
							{#if m.image_name}
								<div class="text-xs text-[var(--color-text-secondary)]">{m.image_name}</div>
							{/if}
						</div>
						<div class="text-[var(--color-text-primary)]">
							<PlatformLabel platform={m.platform} />
						</div>
						<div>
							<span
								class="inline-flex items-center gap-1.5 rounded px-2 py-0.5 text-xs font-medium {statusConfig[
									m.status
								].badge}"
							>
								<span class="inline-block h-1.5 w-1.5 rounded-full {statusConfig[m.status].dot}"
								></span>
								{statusConfig[m.status].label}
							</span>
						</div>
						<div class="font-mono text-xs text-[var(--color-text-secondary)]">
							{m.ip ?? '-'}
						</div>
						<div class="text-xs text-[var(--color-text-secondary)]">
							{m.provider ?? '-'}
						</div>
					</a>
				{/each}
			</div>
		{/if}
	</div>
</div>
