<script lang="ts">
	import type { PageData } from './$types';

	interface Props {
		data: PageData;
	}

	let { data }: Props = $props();
</script>

<div class="mx-auto max-w-7xl space-y-6">
	<h1 class="text-3xl font-semibold text-[var(--color-text-primary)]">Machines</h1>

	<div class="overflow-hidden rounded-2xl bg-[var(--color-bg-secondary)]">
		<div
			class="grid grid-cols-[80px_1fr_140px_140px_200px] gap-4 border-b border-[var(--color-border)] px-6 py-4 text-sm font-medium text-[var(--color-text-secondary)]"
		>
			<div>ID</div>
			<div>Name</div>
			<div>Platform</div>
			<div>Status</div>
			<div>IP</div>
		</div>

		{#if data.machines.length === 0}
			<div class="p-8 text-center text-sm text-[var(--color-text-secondary)]">
				No machines registered.
			</div>
		{:else}
			{#each data.machines as m (m.id ?? m.name)}
				<a
					href={m.id != null ? `/machines/${m.id}` : '#'}
					class="grid grid-cols-[80px_1fr_140px_140px_200px] gap-4 border-b border-[var(--color-border)] px-6 py-4 text-sm transition-colors last:border-b-0 hover:bg-[var(--color-bg-tertiary)]"
				>
					<div class="text-[var(--color-text-secondary)]">{m.id != null ? `#${m.id}` : '—'}</div>
					<div class="text-[var(--color-text-primary)]">{m.name}</div>
					<div class="text-[var(--color-text-primary)]">{m.platform}</div>
					<div class="text-[var(--color-text-primary)]">{m.status}</div>
					<div class="text-[var(--color-text-secondary)]">{m.ip ?? '—'}</div>
				</a>
			{/each}
		{/if}
	</div>
</div>
