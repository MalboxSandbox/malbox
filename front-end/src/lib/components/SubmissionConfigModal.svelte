<script lang="ts">
	import { createDialog } from '@melt-ui/svelte';
	import Icon from '$lib/components/ui/Icon.svelte';
	import { icons } from '$lib/icons';
	import { listAvailablePlugins } from '$lib/api/plugins';
	import { listMachines, listSnapshots } from '$lib/api/machines';
	import type { Platform, AvailablePlugins, Machine, Snapshot } from '$lib/api/types';

	interface SubmissionConfig {
		timeout: number | null;
		platform: Platform | null;
		tags: string[];
		plugins: string[];
		machineId: number | null;
		snapshotId: string | null;
		priority: number;
		vmMode: 'windows' | 'linux' | 'no-vm';
	}

	interface Props {
		config: SubmissionConfig;
		onApply: (config: SubmissionConfig) => void;
	}

	// eslint-disable-next-line svelte/no-unused-props -- platform is output-only, derived from vmMode
	let { config, onApply }: Props = $props();

	let draftTimeout = $state<number | null>(null);
	let draftTags = $state<string[]>([]);
	let draftPlugins = $state<string[]>([]);
	let draftMachineId = $state<number | null>(null);
	let draftSnapshotId = $state<string | null>(null);
	let draftPriority = $state(2);
	let draftVmMode = $state<'windows' | 'linux' | 'no-vm'>('windows');
	let tagInput = $state('');

	let availablePlugins = $state<AvailablePlugins | null>(null);
	let machines = $state<Machine[]>([]);
	let snapshots = $state<Snapshot[]>([]);
	let loadingMachines = $state(false);
	let loadingSnapshots = $state(false);
	let machineDropdownOpen = $state(false);
	let snapshotDropdownOpen = $state(false);

	const {
		elements: { trigger, portalled, overlay, content, title, close },
		states: { open }
	} = createDialog();

	const hasActiveConfig = $derived(
		config.vmMode !== 'windows' ||
			config.priority !== 2 ||
			config.timeout !== null ||
			config.tags.length > 0 ||
			config.plugins.length > 0 ||
			config.machineId !== null ||
			config.snapshotId !== null
	);

	const platformMachines = $derived(
		draftVmMode !== 'no-vm' ? machines.filter((m) => m.platform === draftVmMode) : []
	);

	const selectedMachine = $derived(platformMachines.find((m) => m.id === draftMachineId) ?? null);

	const selectedSnapshot = $derived(snapshots.find((s) => s.id === draftSnapshotId) ?? null);

	const snapshotGuestPluginNames = $derived<string[]>(
		selectedSnapshot?.guest_plugins ? (selectedSnapshot.guest_plugins as string[]) : []
	);

	const visibleGuestPlugins = $derived(
		draftVmMode !== 'no-vm' && selectedSnapshot && availablePlugins
			? availablePlugins.guest.filter((g) => snapshotGuestPluginNames.includes(g.name))
			: []
	);

	$effect(() => {
		if ($open) {
			draftTimeout = config.timeout;
			draftTags = [...config.tags];
			draftPlugins = [...config.plugins];
			draftMachineId = config.machineId;
			draftSnapshotId = config.snapshotId;
			draftPriority = config.priority;
			draftVmMode = config.vmMode;
			tagInput = '';
			loadPlugins();
			loadAllMachines();
		}
	});

	$effect(() => {
		if ($open && draftVmMode === 'no-vm') {
			draftMachineId = null;
			draftSnapshotId = null;
			snapshots = [];
		}
	});

	$effect(() => {
		if ($open && draftMachineId !== null) {
			loadMachineSnapshots(draftMachineId);
		} else if ($open) {
			snapshots = [];
			draftSnapshotId = null;
		}
	});

	async function loadPlugins() {
		try {
			availablePlugins = await listAvailablePlugins(fetch);
		} catch {
			availablePlugins = null;
		}
	}

	async function loadAllMachines() {
		loadingMachines = true;
		try {
			machines = await listMachines(fetch);
		} catch {
			machines = [];
		} finally {
			loadingMachines = false;
		}
	}

	async function loadMachineSnapshots(machineId: number) {
		loadingSnapshots = true;
		try {
			snapshots = await listSnapshots(fetch, machineId);
			if (snapshots.length === 1) {
				selectSnapshot(snapshots[0]);
			} else if (draftSnapshotId && !snapshots.find((s) => s.id === draftSnapshotId)) {
				draftSnapshotId = null;
			}
		} catch {
			snapshots = [];
		} finally {
			loadingSnapshots = false;
		}
	}

	function selectMachine(machineId: number) {
		draftMachineId = machineId > 0 ? machineId : null;
		snapshotDropdownOpen = false;
	}

	function closeDropdowns() {
		machineDropdownOpen = false;
		snapshotDropdownOpen = false;
	}

	function selectSnapshot(snap: Snapshot) {
		draftSnapshotId = snap.id;
		const guestPluginNames: string[] = snap.guest_plugins ? (snap.guest_plugins as string[]) : [];
		if (availablePlugins) {
			const hostNames = availablePlugins.host.map((p) => p.name);
			const guestNames = availablePlugins.guest
				.filter((g) => guestPluginNames.includes(g.name))
				.map((g) => g.name);
			draftPlugins = [...hostNames, ...guestNames];
		}
	}

	function togglePlugin(name: string) {
		if (draftPlugins.includes(name)) {
			draftPlugins = draftPlugins.filter((p) => p !== name);
		} else {
			draftPlugins = [...draftPlugins, name];
		}
	}

	function addTag() {
		const value = tagInput.trim();
		if (value && !draftTags.includes(value)) {
			draftTags = [...draftTags, value];
		}
		tagInput = '';
	}

	function removeTag(tag: string) {
		draftTags = draftTags.filter((t) => t !== tag);
	}

	function handleTagKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' || e.key === ',') {
			e.preventDefault();
			addTag();
		}
	}

	function apply() {
		onApply({
			timeout: draftTimeout,
			platform: draftVmMode !== 'no-vm' ? (draftVmMode as Platform) : null,
			tags: draftTags,
			plugins: draftPlugins,
			machineId: draftMachineId,
			snapshotId: draftSnapshotId,
			priority: draftPriority,
			vmMode: draftVmMode
		});
		open.set(false);
	}

	function reset() {
		draftTimeout = null;
		draftTags = [];
		draftPlugins = [];
		draftMachineId = null;
		draftSnapshotId = null;
		draftPriority = 2;
		draftVmMode = 'windows';
		tagInput = '';
	}

	const priorityLabels: { value: number; label: string }[] = [
		{ value: 1, label: 'Low' },
		{ value: 2, label: 'Normal' },
		{ value: 3, label: 'High' },
		{ value: 4, label: 'Critical' }
	];
</script>

<button
	use:trigger
	type="button"
	class="relative inline-flex items-center justify-center rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] p-2.5 text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-card)] hover:text-[var(--color-text-primary)]"
	title="Submission options"
>
	<Icon path={icons.settings} class="size-4" />
	{#if hasActiveConfig}
		<span class="absolute -top-1 -right-1 size-2.5 rounded-full bg-[var(--color-accent)]"></span>
	{/if}
</button>

<div use:portalled>
	{#if $open}
		<div use:overlay class="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm"></div>
		<div
			use:content
			class="fixed left-1/2 top-1/2 z-50 w-full max-w-lg -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-5 py-4 max-h-[85vh] overflow-y-auto"
			onclick={(e) => {
				const target = e.target as HTMLElement;
				if (!target.closest('[data-dropdown]')) closeDropdowns();
			}}
		>
			<div class="mb-4 flex items-center justify-between">
				<h3 use:title class="text-sm font-semibold text-[var(--color-text-primary)]">
					Submission Options
				</h3>
				<button
					use:close
					aria-label="Close"
					class="rounded-lg p-1 text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-card)] hover:text-[var(--color-text-primary)]"
				>
					<svg class="size-5" viewBox="0 0 20 20" fill="currentColor">
						<path
							d="M6.28 5.22a.75.75 0 00-1.06 1.06L8.94 10l-3.72 3.72a.75.75 0 101.06 1.06L10 11.06l3.72 3.72a.75.75 0 101.06-1.06L11.06 10l3.72-3.72a.75.75 0 00-1.06-1.06L10 8.94 6.28 5.22z"
						/>
					</svg>
				</button>
			</div>

			<div class="space-y-4">
				<!-- VM Mode -->
				<div>
					<span class="mb-1.5 block text-xs font-medium text-[var(--color-text-secondary)]"
						>Platform</span
					>
					<div class="flex gap-1.5">
						<button
							type="button"
							onclick={() => (draftVmMode = 'windows')}
							class="inline-flex flex-1 items-center justify-center gap-1.5 rounded-lg border px-3 py-1.5 text-sm transition-colors {draftVmMode ===
							'windows'
								? 'border-[var(--color-accent)] bg-[var(--color-accent)]/10 text-[var(--color-text-primary)]'
								: 'border-[var(--color-border)] text-[var(--color-text-secondary)] hover:border-[var(--color-text-secondary)]'}"
						>
							<Icon path={icons.windows} viewBox="0 0 24 24" class="size-3.5" />
							Windows
						</button>
						<button
							type="button"
							onclick={() => (draftVmMode = 'linux')}
							class="inline-flex flex-1 items-center justify-center gap-1.5 rounded-lg border px-3 py-1.5 text-sm transition-colors {draftVmMode ===
							'linux'
								? 'border-[var(--color-accent)] bg-[var(--color-accent)]/10 text-[var(--color-text-primary)]'
								: 'border-[var(--color-border)] text-[var(--color-text-secondary)] hover:border-[var(--color-text-secondary)]'}"
						>
							<Icon path={icons.linux} viewBox="0 0 15 15" class="size-3.5" />
							Linux
						</button>
						<button
							type="button"
							onclick={() => (draftVmMode = 'no-vm')}
							class="inline-flex flex-1 items-center justify-center gap-1.5 rounded-lg border px-3 py-1.5 text-sm transition-colors {draftVmMode ===
							'no-vm'
								? 'border-[var(--color-accent)] bg-[var(--color-accent)]/10 text-[var(--color-text-primary)]'
								: 'border-[var(--color-border)] text-[var(--color-text-secondary)] hover:border-[var(--color-text-secondary)]'}"
						>
							No VM
						</button>
					</div>
				</div>

				<!-- Machine + Snapshot dropdowns (only when VM mode is windows/linux) -->
				{#if draftVmMode !== 'no-vm'}
					<div class="flex gap-3">
						<!-- Machine dropdown -->
						<div class="relative flex-1" data-dropdown>
							<span class="mb-1.5 block text-xs font-medium text-[var(--color-text-secondary)]"
								>Machine</span
							>
							{#if loadingMachines}
								<div
									class="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-card)] px-3 py-2 text-sm text-[var(--color-text-secondary)]"
								>
									Loading...
								</div>
							{:else}
								<button
									type="button"
									onclick={() => {
										machineDropdownOpen = !machineDropdownOpen;
										snapshotDropdownOpen = false;
									}}
									class="flex w-full items-center justify-between rounded-lg border bg-[var(--color-bg-card)] px-3 py-2 text-sm transition-colors {machineDropdownOpen
										? 'border-[var(--color-accent)] ring-1 ring-[var(--color-accent)]'
										: 'border-[var(--color-border)]'}"
								>
									{#if selectedMachine}
										<div class="flex items-center gap-2 overflow-hidden">
											<span
												class="size-2 shrink-0 rounded-full {selectedMachine.status === 'ready'
													? 'bg-green-400'
													: selectedMachine.status === 'assigned'
														? 'bg-yellow-400'
														: selectedMachine.status === 'failed'
															? 'bg-red-400'
															: 'bg-[var(--color-text-secondary)]'}"
											></span>
											<span class="truncate text-[var(--color-text-primary)]"
												>{selectedMachine.name}</span
											>
										</div>
									{:else}
										<span class="text-[var(--color-text-secondary)]">Select machine</span>
									{/if}
									<svg
										class="size-4 shrink-0 text-[var(--color-text-secondary)] transition-transform {machineDropdownOpen
											? 'rotate-180'
											: ''}"
										viewBox="0 0 20 20"
										fill="currentColor"
									>
										<path
											fill-rule="evenodd"
											d="M5.22 8.22a.75.75 0 011.06 0L10 11.94l3.72-3.72a.75.75 0 111.06 1.06l-4.25 4.25a.75.75 0 01-1.06 0L5.22 9.28a.75.75 0 010-1.06z"
											clip-rule="evenodd"
										/>
									</svg>
								</button>
								{#if machineDropdownOpen}
									<div
										class="absolute left-0 right-0 top-full z-10 mt-1 max-h-48 overflow-y-auto rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] py-1 shadow-lg"
									>
										{#each platformMachines as machine (machine.id)}
											<button
												type="button"
												onclick={() => {
													selectMachine(machine.id!);
													machineDropdownOpen = false;
												}}
												class="flex w-full items-center gap-2 px-3 py-1.5 text-sm transition-colors {draftMachineId ===
												machine.id
													? 'bg-[var(--color-accent)]/10 text-[var(--color-text-primary)]'
													: 'text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-card)] hover:text-[var(--color-text-primary)]'}"
											>
												<span
													class="size-2 shrink-0 rounded-full {machine.status === 'ready'
														? 'bg-green-400'
														: machine.status === 'assigned'
															? 'bg-yellow-400'
															: machine.status === 'failed'
																? 'bg-red-400'
																: 'bg-[var(--color-text-secondary)]'}"
												></span>
												<span class="truncate">{machine.name}</span>
												<span class="ml-auto shrink-0 text-xs text-[var(--color-text-secondary)]"
													>{machine.status}</span
												>
											</button>
										{/each}
										{#if platformMachines.length === 0}
											<div class="px-3 py-2 text-sm text-[var(--color-text-secondary)]">
												No machines for {draftVmMode}
											</div>
										{/if}
									</div>
								{/if}
							{/if}
						</div>

						<!-- Snapshot dropdown -->
						<div class="relative flex-1" data-dropdown>
							<span class="mb-1.5 block text-xs font-medium text-[var(--color-text-secondary)]"
								>Snapshot</span
							>
							{#if draftMachineId === null}
								<div
									class="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-card)] px-3 py-2 text-sm text-[var(--color-text-secondary)]"
								>
									Pick a machine
								</div>
							{:else if loadingSnapshots}
								<div
									class="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-card)] px-3 py-2 text-sm text-[var(--color-text-secondary)]"
								>
									Loading...
								</div>
							{:else}
								<button
									type="button"
									onclick={() => {
										snapshotDropdownOpen = !snapshotDropdownOpen;
										machineDropdownOpen = false;
									}}
									class="flex w-full items-center justify-between rounded-lg border bg-[var(--color-bg-card)] px-3 py-2 text-sm transition-colors {snapshotDropdownOpen
										? 'border-[var(--color-accent)] ring-1 ring-[var(--color-accent)]'
										: 'border-[var(--color-border)]'}"
								>
									<span
										class="truncate {selectedSnapshot
											? 'text-[var(--color-text-primary)]'
											: 'text-[var(--color-text-secondary)]'}"
									>
										{selectedSnapshot?.name ?? 'Select snapshot'}
									</span>
									<svg
										class="size-4 shrink-0 text-[var(--color-text-secondary)] transition-transform {snapshotDropdownOpen
											? 'rotate-180'
											: ''}"
										viewBox="0 0 20 20"
										fill="currentColor"
									>
										<path
											fill-rule="evenodd"
											d="M5.22 8.22a.75.75 0 011.06 0L10 11.94l3.72-3.72a.75.75 0 111.06 1.06l-4.25 4.25a.75.75 0 01-1.06 0L5.22 9.28a.75.75 0 010-1.06z"
											clip-rule="evenodd"
										/>
									</svg>
								</button>
								{#if snapshotDropdownOpen}
									<div
										class="absolute left-0 right-0 top-full z-10 mt-1 max-h-48 overflow-y-auto rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] py-1 shadow-lg"
									>
										{#each snapshots as snap (snap.id)}
											<button
												type="button"
												onclick={() => {
													selectSnapshot(snap);
													snapshotDropdownOpen = false;
												}}
												class="w-full px-3 py-1.5 text-left text-sm transition-colors {draftSnapshotId ===
												snap.id
													? 'bg-[var(--color-accent)]/10 text-[var(--color-text-primary)]'
													: 'text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-card)] hover:text-[var(--color-text-primary)]'}"
											>
												<span>{snap.name}</span>
												{#if snap.description}
													<span class="ml-1 text-xs text-[var(--color-text-secondary)]"
														>- {snap.description}</span
													>
												{/if}
											</button>
										{/each}
										{#if snapshots.length === 0}
											<div class="px-3 py-2 text-sm text-[var(--color-text-secondary)]">
												No snapshots
											</div>
										{/if}
									</div>
								{/if}
							{/if}
						</div>
					</div>
				{/if}

				<!-- Plugins -->
				{#if availablePlugins && (availablePlugins.host.length > 0 || visibleGuestPlugins.length > 0)}
					<div>
						<span class="mb-1.5 block text-xs font-medium text-[var(--color-text-secondary)]"
							>Plugins</span
						>
						<div
							class="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-card)] p-2 space-y-1.5"
						>
							{#if availablePlugins.host.length > 0}
								<div class="flex flex-wrap items-center gap-1.5">
									<span
										class="text-[10px] font-semibold uppercase tracking-wider text-[var(--color-text-secondary)] mr-0.5"
										>Host</span
									>
									{#each availablePlugins.host as plugin (plugin.name)}
										<label
											class="inline-flex cursor-pointer items-center gap-1.5 rounded-md px-2 py-1 text-xs transition-colors {draftPlugins.includes(
												plugin.name
											)
												? 'bg-[var(--color-accent)]/15 text-[var(--color-text-primary)]'
												: 'text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-tertiary)]'}"
											title={plugin.description ?? plugin.name}
										>
											<input
												type="checkbox"
												checked={draftPlugins.includes(plugin.name)}
												onchange={() => togglePlugin(plugin.name)}
												class="sr-only"
											/>
											<span
												class="flex size-3.5 items-center justify-center rounded border {draftPlugins.includes(
													plugin.name
												)
													? 'border-[var(--color-accent)] bg-[var(--color-accent)]'
													: 'border-[var(--color-border)]'}"
											>
												{#if draftPlugins.includes(plugin.name)}
													<svg class="size-2.5 text-white" viewBox="0 0 12 12" fill="none">
														<path
															d="M2 6l3 3 5-5"
															stroke="currentColor"
															stroke-width="2"
															stroke-linecap="round"
															stroke-linejoin="round"
														/>
													</svg>
												{/if}
											</span>
											{plugin.name}
										</label>
									{/each}
								</div>
							{/if}
							{#if visibleGuestPlugins.length > 0}
								{#if availablePlugins.host.length > 0}
									<div class="border-t border-[var(--color-border)]"></div>
								{/if}
								<div class="flex flex-wrap items-center gap-1.5">
									<span
										class="text-[10px] font-semibold uppercase tracking-wider text-[var(--color-text-secondary)] mr-0.5"
										>Guest</span
									>
									{#each visibleGuestPlugins as plugin (plugin.name)}
										<label
											class="inline-flex cursor-pointer items-center gap-1.5 rounded-md px-2 py-1 text-xs transition-colors {draftPlugins.includes(
												plugin.name
											)
												? 'bg-[var(--color-accent)]/15 text-[var(--color-text-primary)]'
												: 'text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-tertiary)]'}"
											title={plugin.description ?? plugin.name}
										>
											<input
												type="checkbox"
												checked={draftPlugins.includes(plugin.name)}
												onchange={() => togglePlugin(plugin.name)}
												class="sr-only"
											/>
											<span
												class="flex size-3.5 items-center justify-center rounded border {draftPlugins.includes(
													plugin.name
												)
													? 'border-[var(--color-accent)] bg-[var(--color-accent)]'
													: 'border-[var(--color-border)]'}"
											>
												{#if draftPlugins.includes(plugin.name)}
													<svg class="size-2.5 text-white" viewBox="0 0 12 12" fill="none">
														<path
															d="M2 6l3 3 5-5"
															stroke="currentColor"
															stroke-width="2"
															stroke-linecap="round"
															stroke-linejoin="round"
														/>
													</svg>
												{/if}
											</span>
											{plugin.name}
										</label>
									{/each}
								</div>
							{/if}
						</div>
					</div>
				{/if}

				<!-- Priority -->
				<div>
					<span class="mb-1.5 block text-xs font-medium text-[var(--color-text-secondary)]"
						>Priority</span
					>
					<div class="flex gap-1">
						{#each priorityLabels as { value, label } (value)}
							<button
								type="button"
								onclick={() => (draftPriority = value)}
								class="flex-1 rounded-md border px-1.5 py-1.5 text-xs transition-colors {draftPriority ===
								value
									? 'border-[var(--color-accent)] bg-[var(--color-accent)]/10 text-[var(--color-text-primary)]'
									: 'border-[var(--color-border)] text-[var(--color-text-secondary)] hover:border-[var(--color-text-secondary)]'}"
							>
								{label}
							</button>
						{/each}
					</div>
				</div>

				<!-- Timeout -->
				<div>
					<div class="mb-1.5 flex items-center justify-between">
						<span class="text-xs font-medium text-[var(--color-text-secondary)]">Timeout</span>
						<span class="text-xs text-[var(--color-text-secondary)]">
							{draftTimeout !== null ? `${draftTimeout}s` : 'Default'}
						</span>
					</div>
					<input
						type="range"
						min="30"
						max="600"
						step="10"
						value={draftTimeout ?? 300}
						oninput={(e) => {
							draftTimeout = Number((e.currentTarget as HTMLInputElement).value);
						}}
						class="w-full accent-[var(--color-accent)]"
					/>
				</div>

				<!-- Tags input -->
				<div>
					<span class="mb-1.5 block text-xs font-medium text-[var(--color-text-secondary)]"
						>Tags</span
					>
					<div
						class="flex flex-wrap items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-card)] px-2.5 py-1.5"
					>
						{#each draftTags as tag (tag)}
							<span
								class="inline-flex items-center gap-0.5 rounded-full bg-[var(--color-accent)]/15 px-2 py-0.5 text-xs font-medium text-[var(--color-text-primary)]"
							>
								{tag}
								<button
									type="button"
									aria-label="Remove tag {tag}"
									onclick={() => removeTag(tag)}
									class="ml-0.5 rounded-full p-0.5 transition-colors hover:bg-[var(--color-accent)]/25"
								>
									<svg class="size-2.5" viewBox="0 0 20 20" fill="currentColor">
										<path
											d="M6.28 5.22a.75.75 0 00-1.06 1.06L8.94 10l-3.72 3.72a.75.75 0 101.06 1.06L10 11.06l3.72 3.72a.75.75 0 101.06-1.06L11.06 10l3.72-3.72a.75.75 0 00-1.06-1.06L10 8.94 6.28 5.22z"
										/>
									</svg>
								</button>
							</span>
						{/each}
						<input
							type="text"
							bind:value={tagInput}
							onkeydown={handleTagKeydown}
							placeholder={draftTags.length === 0 ? 'Type and press Enter' : ''}
							class="min-w-[100px] flex-1 bg-transparent text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none"
						/>
					</div>
				</div>
			</div>

			<!-- Footer -->
			<div class="mt-4 flex gap-3">
				<button
					type="button"
					onclick={reset}
					class="flex-1 rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm text-[var(--color-text-primary)] transition-colors hover:bg-[var(--color-bg-card)]"
				>
					Reset
				</button>
				<button
					type="button"
					onclick={apply}
					class="flex-1 rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-[var(--color-accent-hover)]"
				>
					Apply
				</button>
			</div>
		</div>
	{/if}
</div>
