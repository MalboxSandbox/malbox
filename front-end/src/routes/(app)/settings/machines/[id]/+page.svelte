<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { deleteSnapshot, provisionMachine } from '$lib/api/machines';
	import { toasts } from '$lib/stores/toasts.svelte';
	import { isApiError } from '$lib/api/errors';
	import PlatformLabel from '$lib/components/ui/PlatformLabel.svelte';
	import type { MachineStatus, ProvisionRunStatus } from '$lib/api/types';
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
	let showProvision = $state(false);

	const statusConfig: Record<MachineStatus, { dot: string; badge: string }> = {
		ready: { dot: 'bg-emerald-400', badge: 'bg-emerald-400/10 text-emerald-400' },
		assigned: {
			dot: 'bg-[var(--color-accent)]',
			badge: 'bg-[var(--color-accent)]/10 text-[var(--color-accent)]'
		},
		creating: { dot: 'bg-amber-400', badge: 'bg-amber-400/10 text-amber-400' },
		provisioning: { dot: 'bg-amber-400', badge: 'bg-amber-400/10 text-amber-400' },
		reverting: { dot: 'bg-amber-400', badge: 'bg-amber-400/10 text-amber-400' },
		deleting: { dot: 'bg-orange-400', badge: 'bg-orange-400/10 text-orange-400' },
		failed: { dot: 'bg-red-400', badge: 'bg-red-400/10 text-red-400' }
	};

	const provisionStatusConfig: Record<ProvisionRunStatus, string> = {
		running: 'bg-amber-400/10 text-amber-400',
		success: 'bg-emerald-400/10 text-emerald-400',
		failed: 'bg-red-400/10 text-red-400'
	};

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
			showProvision = false;
			await invalidate('malbox:machines');
		} catch (err) {
			const msg = isApiError(err) ? err.message : 'Provision failed.';
			toasts.push({ kind: 'error', message: msg });
		} finally {
			provisionBusy = false;
		}
	}

	const detailFields = $derived([
		{ label: 'ID', value: data.machine.id ?? '-' },
		{ label: 'Platform', value: null, slot: 'platform' },
		{ label: 'Arch', value: data.machine.arch },
		{ label: 'IP', value: data.machine.ip ?? '-', mono: true },
		{ label: 'Provider', value: data.machine.provider ?? '-' },
		{ label: 'Image', value: data.machine.image_name ?? '-' },
		{ label: 'Current task', value: data.machine.current_task_id ?? '-' },
		{ label: 'Last seen', value: data.machine.last_seen ?? '-' }
	]);
</script>

<div class="mx-auto max-w-5xl space-y-6">
	<div class="flex items-center gap-3 text-sm text-[var(--color-text-secondary)]">
		<a href="/settings" class="hover:text-[var(--color-text-primary)]">Settings</a>
		<span>/</span>
		<a href="/settings/machines" class="hover:text-[var(--color-text-primary)]">Machines</a>
		<span>/</span>
		<span class="text-[var(--color-text-primary)]">{data.machine.name}</span>
	</div>

	<div class="rounded-2xl bg-[var(--color-bg-secondary)] p-8">
		<div class="flex items-start justify-between gap-4">
			<div>
				<h1 class="text-2xl font-semibold text-[var(--color-text-primary)]">
					{data.machine.name}
				</h1>
				{#if data.machine.label}
					<p class="mt-1 text-sm text-[var(--color-text-secondary)]">{data.machine.label}</p>
				{/if}
			</div>
			<span
				class="inline-flex items-center gap-2 rounded-lg px-3 py-1.5 text-sm font-medium {statusConfig[
					data.machine.status
				].badge}"
			>
				<span class="inline-block h-2 w-2 rounded-full {statusConfig[data.machine.status].dot}"
				></span>
				{data.machine.status}
			</span>
		</div>

		{#if data.machine.error_message}
			<div
				class="mt-4 rounded-lg border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-300"
			>
				{data.machine.error_message}
			</div>
		{/if}

		<div class="mt-6 grid grid-cols-2 gap-x-8 gap-y-4 text-sm md:grid-cols-4">
			{#each detailFields as field (field.label)}
				<div>
					<dt class="text-xs font-medium text-[var(--color-text-secondary)]">{field.label}</dt>
					<dd
						class="mt-0.5 text-[var(--color-text-primary)] {field.mono ? 'font-mono text-xs' : ''}"
					>
						{#if field.slot === 'platform'}
							<PlatformLabel platform={data.machine.platform} />
						{:else}
							{field.value}
						{/if}
					</dd>
				</div>
			{/each}
		</div>

		{#if data.machine.cpus || data.machine.memory_mb || data.machine.disk_size_mb}
			<div class="mt-6 flex gap-6 border-t border-[var(--color-border)] pt-4">
				{#if data.machine.cpus}
					<div class="text-sm">
						<span class="text-[var(--color-text-secondary)]">CPUs</span>
						<span class="ml-2 text-[var(--color-text-primary)]">{data.machine.cpus}</span>
					</div>
				{/if}
				{#if data.machine.memory_mb}
					<div class="text-sm">
						<span class="text-[var(--color-text-secondary)]">Memory</span>
						<span class="ml-2 text-[var(--color-text-primary)]">{data.machine.memory_mb} MB</span>
					</div>
				{/if}
				{#if data.machine.disk_size_mb}
					<div class="text-sm">
						<span class="text-[var(--color-text-secondary)]">Disk</span>
						<span class="ml-2 text-[var(--color-text-primary)]">{data.machine.disk_size_mb} MB</span
						>
					</div>
				{/if}
			</div>
		{/if}
	</div>

	<div class="rounded-2xl bg-[var(--color-bg-secondary)] p-8">
		<div class="flex items-center justify-between">
			<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">Snapshots</h2>
			<span class="text-xs text-[var(--color-text-secondary)]">{data.snapshots.length} total</span>
		</div>
		{#if data.snapshots.length === 0}
			<p class="mt-4 text-sm text-[var(--color-text-secondary)]">No snapshots.</p>
		{:else}
			<div class="mt-4 space-y-2">
				{#each data.snapshots as s (s.id)}
					<div
						class="flex items-center justify-between rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3"
					>
						<div class="flex items-center gap-3">
							<span class="text-sm text-[var(--color-text-primary)]">{s.name}</span>
							{#if s.is_active}
								<span
									class="rounded bg-emerald-400/10 px-1.5 py-0.5 text-xs font-medium text-emerald-400"
									>active</span
								>
							{/if}
							{#if s.description}
								<span class="text-xs text-[var(--color-text-secondary)]">{s.description}</span>
							{/if}
						</div>
						<button
							class="rounded-lg px-3 py-1 text-xs text-red-400 transition-colors hover:bg-red-400/10 hover:text-red-300"
							onclick={() => handleDeleteSnapshot(s.name)}
						>
							Delete
						</button>
					</div>
				{/each}
			</div>
		{/if}
	</div>

	<div class="rounded-2xl bg-[var(--color-bg-secondary)] p-8">
		<div class="flex items-center justify-between">
			<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">Provisioning</h2>
			<button
				onclick={() => (showProvision = !showProvision)}
				class="rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white transition-colors hover:opacity-90"
			>
				{showProvision ? 'Cancel' : 'Run provisioning'}
			</button>
		</div>

		{#if showProvision}
			<form
				onsubmit={handleProvision}
				class="mt-6 space-y-4 rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] p-6"
			>
				<div class="grid grid-cols-1 gap-4 md:grid-cols-3">
					<div>
						<label for="prov-engine" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
							>Provisioner</label
						>
						<input
							id="prov-engine"
							type="text"
							bind:value={provisioner}
							required
							class="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-secondary)] px-3 py-2 text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
						/>
					</div>
					<div>
						<label for="snap-name" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
							>Snapshot to create</label
						>
						<input
							id="snap-name"
							type="text"
							bind:value={snapshotName}
							placeholder="optional"
							class="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-secondary)] px-3 py-2 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
						/>
					</div>
					<div>
						<label for="revert-to" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
							>Revert to</label
						>
						<input
							id="revert-to"
							type="text"
							bind:value={revertTo}
							placeholder="base"
							class="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-secondary)] px-3 py-2 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
						/>
					</div>
				</div>

				{#if data.guestPlugins.length > 0}
					<div>
						<div class="mb-2 text-xs text-[var(--color-text-secondary)]">Guest plugins</div>
						<div class="flex flex-wrap gap-2">
							{#each data.guestPlugins as p (p.name)}
								<label
									class="flex items-center gap-1.5 rounded-lg bg-[var(--color-bg-secondary)] px-3 py-1.5 text-xs text-[var(--color-text-primary)] transition-colors hover:bg-[var(--color-bg-card)]"
								>
									<input
										type="checkbox"
										bind:group={selectedPlugins}
										value={p.name}
										class="rounded"
									/>
									{p.name}
								</label>
							{/each}
						</div>
					</div>
				{/if}

				<button
					type="submit"
					class="rounded-lg bg-[var(--color-accent)] px-6 py-2 text-sm font-medium text-white transition-colors hover:bg-[var(--color-accent-hover)] disabled:opacity-50"
					disabled={provisionBusy}
				>
					{provisionBusy ? 'Starting...' : 'Start provisioning'}
				</button>
			</form>
		{/if}

		{#if data.provisions.length > 0}
			<div class="mt-6 overflow-hidden rounded-lg border border-[var(--color-border)]">
				<div
					class="grid grid-cols-[1fr_100px_1fr_80px] gap-4 border-b border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-4 py-2 text-xs font-medium text-[var(--color-text-secondary)]"
				>
					<div>Provisioner</div>
					<div>Status</div>
					<div>Time</div>
					<div></div>
				</div>
				{#each data.provisions as r (r.id)}
					<div
						class="grid grid-cols-[1fr_100px_1fr_80px] items-center gap-4 border-b border-[var(--color-border)] px-4 py-3 text-sm last:border-b-0"
					>
						<div class="text-[var(--color-text-primary)]">{r.provisioner}</div>
						<div>
							<span
								class="rounded px-2 py-0.5 text-xs font-medium {provisionStatusConfig[r.status]}"
							>
								{r.status}
							</span>
						</div>
						<div class="text-xs text-[var(--color-text-secondary)]">
							{r.created_at ?? ''}
							{r.updated_at ? ` - ${r.updated_at}` : ''}
						</div>
						<div>
							{#if r.error_message}
								<span class="text-xs text-red-400" title={r.error_message}>error</span>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		{:else if !showProvision}
			<p class="mt-4 text-sm text-[var(--color-text-secondary)]">No provision runs.</p>
		{/if}
	</div>
</div>
