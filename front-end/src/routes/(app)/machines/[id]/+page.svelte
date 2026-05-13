<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { deleteSnapshot, provisionMachine } from '$lib/api/machines';
	import { toasts } from '$lib/stores/toasts.svelte';
	import { isApiError } from '$lib/api/errors';
	import PlatformLabel from '$lib/components/ui/PlatformLabel.svelte';
	import type { PageData } from './$types';

	interface Props {
		data: PageData;
	}

	let { data }: Props = $props();

	let provisioner = $state('ansible');
	let snapshotName = $state('');
	let revertTo = $state('');
	let selectedPlugins = $state<string[]>([]);
	let provisionBusy = $state(false);

	async function handleDeleteSnapshot(name: string) {
		if (!confirm(`Delete snapshot "${name}"?`)) return;
		if (data.machine.id == null) return;
		try {
			await deleteSnapshot(fetch, data.machine.id, name);
			toasts.push({ kind: 'success', message: `Snapshot "${name}" deleted.` });
			await invalidate('malbox:machines');
		} catch (err) {
			const msg = isApiError(err) ? err.message : 'Delete failed.';
			toasts.push({ kind: 'error', message: msg });
		}
	}

	async function handleProvision(e: SubmitEvent) {
		e.preventDefault();
		if (provisionBusy || data.machine.id == null) return;
		provisionBusy = true;
		try {
			await provisionMachine(fetch, data.machine.id, {
				provisioner,
				snapshot: snapshotName || undefined,
				revert_to: revertTo || undefined,
				plugins: selectedPlugins.length ? selectedPlugins : undefined
			});
			toasts.push({ kind: 'success', message: 'Provision run started.' });
			snapshotName = '';
			revertTo = '';
			selectedPlugins = [];
			await invalidate('malbox:machines');
		} catch (err) {
			const msg = isApiError(err) ? err.message : 'Provision failed.';
			toasts.push({ kind: 'error', message: msg });
		} finally {
			provisionBusy = false;
		}
	}
</script>

<div class="mx-auto max-w-5xl space-y-6">
	<a
		href="/machines"
		class="inline-flex items-center gap-2 text-sm text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]"
	>
		← All machines
	</a>

	<div class="space-y-4 rounded-2xl bg-[var(--color-bg-secondary)] p-8">
		<h1 class="text-2xl font-semibold text-[var(--color-text-primary)]">
			{data.machine.name}
		</h1>
		<dl class="grid grid-cols-2 gap-4 text-sm md:grid-cols-4">
			<div>
				<dt class="text-[var(--color-text-secondary)]">ID</dt>
				<dd class="text-[var(--color-text-primary)]">{data.machine.id ?? '—'}</dd>
			</div>
			<div>
				<dt class="text-[var(--color-text-secondary)]">Platform</dt>
				<dd class="text-[var(--color-text-primary)]">
					<PlatformLabel platform={data.machine.platform} />
				</dd>
			</div>
			<div>
				<dt class="text-[var(--color-text-secondary)]">Arch</dt>
				<dd class="text-[var(--color-text-primary)]">{data.machine.arch}</dd>
			</div>
			<div>
				<dt class="text-[var(--color-text-secondary)]">Status</dt>
				<dd class="text-[var(--color-text-primary)]">{data.machine.status}</dd>
			</div>
			<div>
				<dt class="text-[var(--color-text-secondary)]">IP</dt>
				<dd class="text-[var(--color-text-primary)]">{data.machine.ip ?? '—'}</dd>
			</div>
			<div>
				<dt class="text-[var(--color-text-secondary)]">Provider</dt>
				<dd class="text-[var(--color-text-primary)]">{data.machine.provider ?? '—'}</dd>
			</div>
			<div>
				<dt class="text-[var(--color-text-secondary)]">Image</dt>
				<dd class="text-[var(--color-text-primary)]">{data.machine.image_name ?? '—'}</dd>
			</div>
			<div>
				<dt class="text-[var(--color-text-secondary)]">Current task</dt>
				<dd class="text-[var(--color-text-primary)]">{data.machine.current_task_id ?? '—'}</dd>
			</div>
		</dl>
		{#if data.machine.error_message}
			<div class="rounded border border-red-500/40 bg-red-500/10 p-3 text-sm text-red-300">
				{data.machine.error_message}
			</div>
		{/if}
	</div>

	<div class="space-y-4 rounded-2xl bg-[var(--color-bg-secondary)] p-8">
		<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">Snapshots</h2>
		{#if data.snapshots.length === 0}
			<p class="text-sm text-[var(--color-text-secondary)]">No snapshots.</p>
		{:else}
			<ul class="space-y-2">
				{#each data.snapshots as s (s.id)}
					<li
						class="flex items-center justify-between rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3"
					>
						<div>
							<span class="text-sm text-[var(--color-text-primary)]">{s.name}</span>
							{#if s.is_active}
								<span
									class="ml-2 rounded bg-[var(--color-accent)]/20 px-1.5 py-0.5 text-xs text-[var(--color-accent)]"
									>active</span
								>
							{/if}
							{#if s.description}
								<span class="ml-2 text-xs text-[var(--color-text-secondary)]">{s.description}</span>
							{/if}
						</div>
						<button
							class="text-xs text-red-400 hover:text-red-300"
							onclick={() => handleDeleteSnapshot(s.name)}
						>
							Delete
						</button>
					</li>
				{/each}
			</ul>
		{/if}
	</div>

	<div class="space-y-4 rounded-2xl bg-[var(--color-bg-secondary)] p-8">
		<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">Run provisioning</h2>
		<form onsubmit={handleProvision} class="space-y-4">
			<div class="grid grid-cols-1 gap-4 md:grid-cols-3">
				<label class="flex flex-col gap-1">
					<span class="text-xs text-[var(--color-text-secondary)]">Provisioner</span>
					<input
						type="text"
						bind:value={provisioner}
						required
						class="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm text-[var(--color-text-primary)]"
					/>
				</label>
				<label class="flex flex-col gap-1">
					<span class="text-xs text-[var(--color-text-secondary)]"
						>Snapshot to create (optional)</span
					>
					<input
						type="text"
						bind:value={snapshotName}
						class="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm text-[var(--color-text-primary)]"
					/>
				</label>
				<label class="flex flex-col gap-1">
					<span class="text-xs text-[var(--color-text-secondary)]">Revert to (optional)</span>
					<input
						type="text"
						bind:value={revertTo}
						placeholder="base"
						class="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm text-[var(--color-text-primary)]"
					/>
				</label>
			</div>

			<div>
				<div class="mb-1 text-xs text-[var(--color-text-secondary)]">Guest plugins</div>
				{#if data.guestPlugins.length === 0}
					<p class="text-xs text-[var(--color-text-secondary)]">No guest plugins available.</p>
				{:else}
					<div class="flex flex-wrap gap-2">
						{#each data.guestPlugins as p (p.name)}
							<label
								class="flex items-center gap-1 rounded bg-[var(--color-bg-tertiary)] px-2 py-1 text-xs"
							>
								<input type="checkbox" bind:group={selectedPlugins} value={p.name} />
								{p.name}
							</label>
						{/each}
					</div>
				{/if}
			</div>

			<button
				type="submit"
				class="rounded-lg bg-[var(--color-accent)] px-6 py-2 text-sm font-medium text-white transition-colors hover:bg-[var(--color-accent-hover)] disabled:opacity-50"
				disabled={provisionBusy}
			>
				{provisionBusy ? 'Starting…' : 'Start provisioning'}
			</button>
		</form>
	</div>

	<div class="space-y-4 rounded-2xl bg-[var(--color-bg-secondary)] p-8">
		<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">Provision history</h2>
		{#if data.provisions.length === 0}
			<p class="text-sm text-[var(--color-text-secondary)]">No provision runs.</p>
		{:else}
			<ul class="space-y-2">
				{#each data.provisions as r (r.id)}
					<li class="rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 text-sm">
						<div class="flex items-center justify-between">
							<span class="text-[var(--color-text-primary)]">{r.provisioner}</span>
							<span
								class="text-xs {r.status === 'success'
									? 'text-[var(--color-accent)]'
									: r.status === 'failed'
										? 'text-red-400'
										: 'text-[var(--color-text-secondary)]'}"
							>
								{r.status}
							</span>
						</div>
						<div class="text-xs text-[var(--color-text-secondary)]">
							{r.created_at ?? ''}
							{r.updated_at ? `→ ${r.updated_at}` : ''}
						</div>
						{#if r.error_message}
							<div class="mt-1 text-xs text-red-400">{r.error_message}</div>
						{/if}
					</li>
				{/each}
			</ul>
		{/if}
	</div>
</div>
