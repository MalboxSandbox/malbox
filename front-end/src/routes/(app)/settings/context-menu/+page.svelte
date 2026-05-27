<script lang="ts">
	import { lookupProviders } from '$lib/stores/lookupProviders.svelte';
	import { contextMenuSettings } from '$lib/stores/contextMenuSettings.svelte';
	import { transformStore } from '$lib/stores/transforms.svelte';
	import { listRecipes } from '$lib/api/recipes';
	import { GlobeIcon } from '@lucide/svelte';
	import type {
		LookupProvider,
		CustomAction,
		CustomActionType,
		ContextType
	} from '$lib/components/context-menu/types';
	import type { Recipe } from '$lib/api/types';

	type Tab = 'providers' | 'behavior';
	let activeTab = $state<Tab>('providers');

	const tabs: { value: Tab; label: string }[] = [
		{ value: 'providers', label: 'Lookup Providers' },
		{ value: 'behavior', label: 'Behavior' }
	];

	// --- Lookup Providers state ---
	let providerEditing = $state<LookupProvider | null>(null);
	let showProviderForm = $state(false);
	let providerFormName = $state('');
	let providerFormUrl = $state('');
	let providerFormTypes = $state<string[]>([]);
	let providerFormEnabled = $state(true);

	const allIndicatorTypes = ['sha256', 'sha1', 'md5', 'ip', 'domain', 'url', 'email', 'filename'];

	function openAddProvider() {
		providerEditing = null;
		providerFormName = '';
		providerFormUrl = '';
		providerFormTypes = [];
		providerFormEnabled = true;
		showProviderForm = true;
	}

	function openEditProvider(provider: LookupProvider) {
		providerEditing = provider;
		providerFormName = provider.name;
		providerFormUrl = provider.urlTemplate;
		providerFormTypes = [...provider.indicatorTypes];
		providerFormEnabled = provider.enabled;
		showProviderForm = true;
	}

	function toggleProviderType(t: string) {
		if (providerFormTypes.includes(t)) {
			providerFormTypes = providerFormTypes.filter((x) => x !== t);
		} else {
			providerFormTypes = [...providerFormTypes, t];
		}
	}

	function saveProvider() {
		if (!providerFormName.trim() || !providerFormUrl.trim() || providerFormTypes.length === 0)
			return;
		if (providerEditing) {
			lookupProviders.update(providerEditing.id, {
				name: providerFormName.trim(),
				urlTemplate: providerFormUrl.trim(),
				indicatorTypes: providerFormTypes,
				enabled: providerFormEnabled
			});
		} else {
			lookupProviders.add({
				name: providerFormName.trim(),
				urlTemplate: providerFormUrl.trim(),
				indicatorTypes: providerFormTypes,
				enabled: providerFormEnabled
			});
		}
		showProviderForm = false;
	}

	// --- Behavior state ---
	const allDefangTypes = ['ip', 'domain', 'url', 'email', 'sha256', 'sha1', 'md5', 'filename'];
	const dotOptions: { value: '[.]' | '(.)' | '{.}'; label: string }[] = [
		{ value: '[.]', label: '[.]' },
		{ value: '(.)', label: '(.)' },
		{ value: '{.}', label: '{.}' }
	];
	const protocolOptions: { value: 'hxxp' | 'hXXp'; label: string }[] = [
		{ value: 'hxxp', label: 'hxxp:// / hxxps://' },
		{ value: 'hXXp', label: 'hXXp:// / hXXps://' }
	];

	function toggleDefangType(t: string) {
		const current = contextMenuSettings.settings.defangTypes;
		if (current.includes(t)) {
			contextMenuSettings.update({ defangTypes: current.filter((x) => x !== t) });
		} else {
			contextMenuSettings.update({ defangTypes: [...current, t] });
		}
	}

	const defangPreview = $derived.by(() => {
		const s = contextMenuSettings.settings;
		const input = 'https://malware.example.com';
		const prefix = s.protocolReplacement === 'hxxp' ? 'hxxp' : 'hXXp';
		return input
			.replace(/\./g, s.dotReplacement)
			.replace(/^https:\/\//i, `${prefix}s://`)
			.replace(/^http:\/\//i, `${prefix}://`);
	});

	// --- Custom Actions state ---
	let actionEditing = $state<CustomAction | null>(null);
	let showActionForm = $state(false);
	let actionFormName = $state('');
	let actionFormType = $state<CustomActionType>('open-url');
	let actionFormUrl = $state('');
	let actionFormTransformId = $state('');
	let actionFormRecipeId = $state('');
	let actionFormContextTypes = $state<ContextType[]>([]);
	let actionFormEnabled = $state(true);

	let recipes = $state<Recipe[]>([]);
	let recipesLoaded = $state(false);

	async function loadRecipes() {
		if (recipesLoaded) return;
		try {
			recipes = await listRecipes(fetch);
			recipesLoaded = true;
		} catch {
			recipes = [];
		}
	}

	const actionTypeOptions: { value: CustomActionType; label: string }[] = [
		{ value: 'open-url', label: 'Open URL' },
		{ value: 'copy', label: 'Copy to Clipboard' },
		{ value: 'run-transform', label: 'Run Transform' },
		{ value: 'run-recipe', label: 'Run Recipe' },
		{ value: 'send-to-workbench', label: 'Send to Workbench' }
	];

	const allContextTypes: ContextType[] = [
		'hash',
		'indicator',
		'table-cell',
		'artifact',
		'task',
		'code-block',
		'generic'
	];

	const defaultContextTypes: Record<CustomActionType, ContextType[]> = {
		'open-url': ['hash', 'indicator'],
		copy: ['hash', 'indicator', 'table-cell', 'code-block', 'generic'],
		'run-transform': ['indicator', 'table-cell', 'code-block', 'generic'],
		'run-recipe': ['indicator', 'table-cell', 'code-block', 'generic'],
		'send-to-workbench': ['hash', 'indicator', 'artifact', 'code-block']
	};

	function onActionTypeChange(newType: CustomActionType) {
		actionFormType = newType;
		if (!actionEditing) {
			actionFormContextTypes = [...defaultContextTypes[newType]];
		}
	}

	function openAddAction() {
		actionEditing = null;
		actionFormName = '';
		actionFormType = 'open-url';
		actionFormUrl = '';
		actionFormTransformId = '';
		actionFormRecipeId = '';
		actionFormContextTypes = [...defaultContextTypes['open-url']];
		actionFormEnabled = true;
		showActionForm = true;
		loadRecipes();
	}

	function openEditAction(action: CustomAction) {
		actionEditing = action;
		actionFormName = action.name;
		actionFormType = action.actionType;
		actionFormUrl = action.urlTemplate ?? '';
		actionFormTransformId = action.transformId ?? '';
		actionFormRecipeId = action.recipeId ?? '';
		actionFormContextTypes = [...action.contextTypes];
		actionFormEnabled = action.enabled;
		showActionForm = true;
		loadRecipes();
	}

	function toggleContextType(t: ContextType) {
		if (actionFormContextTypes.includes(t)) {
			actionFormContextTypes = actionFormContextTypes.filter((x) => x !== t);
		} else {
			actionFormContextTypes = [...actionFormContextTypes, t];
		}
	}

	function saveAction() {
		if (!actionFormName.trim() || actionFormContextTypes.length === 0) return;
		if (actionFormType === 'open-url' && !actionFormUrl.trim()) return;
		if (actionFormType === 'run-transform' && !actionFormTransformId) return;
		if (actionFormType === 'run-recipe' && !actionFormRecipeId) return;
		const payload: Omit<CustomAction, 'id'> = {
			name: actionFormName.trim(),
			actionType: actionFormType,
			urlTemplate: actionFormType === 'open-url' ? actionFormUrl.trim() : undefined,
			transformId:
				actionFormType === 'run-transform' || actionFormType === 'send-to-workbench'
					? actionFormTransformId || undefined
					: undefined,
			recipeId: actionFormType === 'run-recipe' ? actionFormRecipeId : undefined,
			contextTypes: actionFormContextTypes,
			enabled: actionFormEnabled
		};
		if (actionEditing) {
			contextMenuSettings.updateAction(actionEditing.id, payload);
		} else {
			contextMenuSettings.addAction(payload);
		}
		showActionForm = false;
	}

	const actionSaveDisabled = $derived(
		!actionFormName.trim() ||
			actionFormContextTypes.length === 0 ||
			(actionFormType === 'open-url' && !actionFormUrl.trim()) ||
			(actionFormType === 'run-transform' && !actionFormTransformId) ||
			(actionFormType === 'run-recipe' && !actionFormRecipeId)
	);
</script>

<div class="mx-auto max-w-7xl space-y-6">
	<!-- Breadcrumb -->
	<div class="flex items-center gap-3 text-sm text-[var(--color-text-secondary)]">
		<a href="/settings" class="hover:text-[var(--color-text-primary)]">Settings</a>
		<span>/</span>
		<span class="text-[var(--color-text-primary)]">Context Menu</span>
	</div>

	<!-- Header -->
	<div>
		<h1 class="text-3xl font-semibold text-[var(--color-text-primary)]">Context Menu</h1>
		<p class="mt-1 text-sm text-[var(--color-text-secondary)]">
			Configure the right-click context menu for indicators, artifacts, and other elements.
		</p>
	</div>

	<!-- Tabs -->
	<div class="flex items-center rounded-xl bg-[var(--color-bg-tertiary)] p-1.5">
		{#each tabs as tab (tab.value)}
			<button
				onclick={() => (activeTab = tab.value)}
				class="rounded-lg px-5 py-2.5 text-sm font-medium transition-colors {activeTab === tab.value
					? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
			>
				{tab.label}
			</button>
		{/each}
	</div>

	<!-- ==================== LOOKUP PROVIDERS TAB ==================== -->
	{#if activeTab === 'providers'}
		<div class="flex items-center justify-between">
			<div>
				<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">Lookup Providers</h2>
				<p class="mt-0.5 text-sm text-[var(--color-text-secondary)]">
					External services used for indicator lookups in the context menu.
				</p>
			</div>
			<div class="flex items-center gap-3">
				<button
					type="button"
					class="rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2 text-sm text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text-primary)]"
					onclick={() => lookupProviders.reset()}
				>
					Reset to defaults
				</button>
				<button
					type="button"
					class="rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-[var(--color-accent-hover)]"
					onclick={openAddProvider}
				>
					+ Add Provider
				</button>
			</div>
		</div>

		<div class="grid grid-cols-1 gap-4">
			{#each lookupProviders.providers as provider (provider.id)}
				<div
					class="rounded-2xl bg-[var(--color-bg-secondary)] p-5 transition-colors {provider.enabled
						? ''
						: 'opacity-50'}"
				>
					<div class="flex items-start justify-between gap-4">
						<div class="min-w-0 flex-1">
							<div class="flex items-center gap-3">
								{#if provider.favicon}
									<img
										src={provider.favicon}
										alt=""
										class="size-5 shrink-0 rounded-sm"
										onerror={(e) => {
											(e.currentTarget as HTMLImageElement).style.display = 'none';
											(e.currentTarget as HTMLImageElement).nextElementSibling?.classList.remove(
												'hidden'
											);
										}}
									/>
									<GlobeIcon class="hidden size-5 shrink-0 text-[var(--color-text-secondary)]" />
								{:else}
									<GlobeIcon class="size-5 shrink-0 text-[var(--color-text-secondary)]" />
								{/if}
								<h3 class="text-base font-semibold text-[var(--color-text-primary)]">
									{provider.name}
								</h3>
								{#if !provider.enabled}
									<span
										class="rounded bg-[var(--color-bg-tertiary)] px-2 py-0.5 text-xs text-[var(--color-text-secondary)]"
									>
										Disabled
									</span>
								{/if}
							</div>
							<p
								class="mt-1 truncate font-mono text-xs text-[var(--color-text-secondary)]"
								title={provider.urlTemplate}
							>
								{provider.urlTemplate}
							</p>
							<div class="mt-3 flex flex-wrap gap-1.5">
								{#each provider.indicatorTypes as t (t)}
									<span
										class="rounded-full bg-[var(--color-accent)]/15 px-2.5 py-0.5 text-xs font-medium text-[var(--color-accent)]"
									>
										{t}
									</span>
								{/each}
							</div>
						</div>
						<div class="flex items-center gap-2">
							<button
								type="button"
								class="rounded-lg px-3 py-1.5 text-sm text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-primary)]"
								onclick={() => lookupProviders.update(provider.id, { enabled: !provider.enabled })}
							>
								{provider.enabled ? 'Disable' : 'Enable'}
							</button>
							<button
								type="button"
								class="rounded-lg px-3 py-1.5 text-sm text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-primary)]"
								onclick={() => openEditProvider(provider)}
							>
								Edit
							</button>
							<button
								type="button"
								class="rounded-lg px-3 py-1.5 text-sm text-red-400 transition-colors hover:bg-red-500/10"
								onclick={() => lookupProviders.remove(provider.id)}
							>
								Delete
							</button>
						</div>
					</div>
				</div>
			{/each}
		</div>

		{#if lookupProviders.providers.length === 0}
			<div
				class="rounded-2xl bg-[var(--color-bg-secondary)] p-8 text-center text-sm text-[var(--color-text-secondary)]"
			>
				No providers configured. Click "Add Provider" or "Reset to defaults".
			</div>
		{/if}

		<!-- Provider Add/Edit Modal -->
		{#if showProviderForm}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<!-- svelte-ignore a11y_click_events_have_key_events -->
			<div
				class="fixed inset-0 z-40 flex items-center justify-center bg-black/50 backdrop-blur-sm"
				onclick={() => (showProviderForm = false)}
			>
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<div
					class="w-full max-w-lg rounded-2xl bg-[var(--color-bg-secondary)] p-6 shadow-2xl"
					onclick={(e) => e.stopPropagation()}
				>
					<h2 class="mb-4 text-lg font-semibold text-[var(--color-text-primary)]">
						{providerEditing ? 'Edit Provider' : 'Add Provider'}
					</h2>

					<div class="space-y-4">
						<div>
							<label
								for="provider-form-name"
								class="mb-1 block text-xs font-medium text-[var(--color-text-secondary)]"
								>Name</label
							>
							<input
								id="provider-form-name"
								type="text"
								bind:value={providerFormName}
								placeholder="e.g. VirusTotal"
								class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2.5 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
							/>
						</div>

						<div>
							<label
								for="provider-form-url"
								class="mb-1 block text-xs font-medium text-[var(--color-text-secondary)]"
							>
								URL template
								<span class="font-normal opacity-70">- use {'{value}'} as placeholder</span>
							</label>
							<input
								id="provider-form-url"
								type="text"
								bind:value={providerFormUrl}
								placeholder="https://example.com/search?q={'{value}'}"
								class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2.5 font-mono text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
							/>
						</div>

						<div>
							<span class="mb-2 block text-xs font-medium text-[var(--color-text-secondary)]"
								>Indicator types</span
							>
							<div class="flex flex-wrap gap-2">
								{#each allIndicatorTypes as t (t)}
									<button
										type="button"
										class="rounded-full px-3 py-1 text-xs font-medium transition-colors
											{providerFormTypes.includes(t)
											? 'bg-[var(--color-accent)] text-white'
											: 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
										onclick={() => toggleProviderType(t)}
									>
										{t}
									</button>
								{/each}
							</div>
						</div>

						<div class="flex items-center gap-2">
							<button
								type="button"
								class="size-5 rounded border transition-colors
									{providerFormEnabled
									? 'border-[var(--color-accent)] bg-[var(--color-accent)]'
									: 'border-[var(--color-border)] bg-[var(--color-bg-tertiary)]'}"
								onclick={() => (providerFormEnabled = !providerFormEnabled)}
							>
								{#if providerFormEnabled}
									<svg
										xmlns="http://www.w3.org/2000/svg"
										viewBox="0 0 20 20"
										fill="currentColor"
										class="size-5 text-white"
									>
										<path
											fill-rule="evenodd"
											d="M16.704 4.153a.75.75 0 0 1 .143 1.052l-8 10.5a.75.75 0 0 1-1.127.075l-4.5-4.5a.75.75 0 0 1 1.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 0 1 1.05-.143Z"
											clip-rule="evenodd"
										/>
									</svg>
								{/if}
							</button>
							<span class="text-sm text-[var(--color-text-primary)]">Enabled</span>
						</div>
					</div>

					<div class="mt-6 flex justify-end gap-3">
						<button
							type="button"
							class="rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2 text-sm text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text-primary)]"
							onclick={() => (showProviderForm = false)}
						>
							Cancel
						</button>
						<button
							type="button"
							class="rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-[var(--color-accent-hover)] disabled:opacity-50"
							disabled={!providerFormName.trim() ||
								!providerFormUrl.trim() ||
								providerFormTypes.length === 0}
							onclick={saveProvider}
						>
							{providerEditing ? 'Save Changes' : 'Add Provider'}
						</button>
					</div>
				</div>
			</div>
		{/if}

		<!-- ==================== BEHAVIOR TAB ==================== -->
	{:else if activeTab === 'behavior'}
		<!-- Menu Actions -->
		<div class="flex items-center justify-between">
			<div>
				<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">Menu Actions</h2>
				<p class="mt-0.5 text-sm text-[var(--color-text-secondary)]">
					Configure which actions appear in the context menu.
				</p>
			</div>
			<button
				type="button"
				class="rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-[var(--color-accent-hover)]"
				onclick={openAddAction}
			>
				+ Add Action
			</button>
		</div>
		<div class="rounded-2xl bg-[var(--color-bg-secondary)] p-5">
			<!-- Built-in toggles -->
			<div
				class="mb-3 text-[10px] font-medium uppercase tracking-wider text-[var(--color-text-secondary)]"
			>
				Built-in
			</div>
			<div class="divide-y divide-[var(--color-border)]/20">
				<div class="flex items-center justify-between py-3 first:pt-0 last:pb-0">
					<div>
						<div class="text-sm font-medium text-[var(--color-text-primary)]">Copy (defanged)</div>
						<div class="mt-0.5 text-xs text-[var(--color-text-secondary)]">
							Show a "Copy (defanged)" option for indicators
						</div>
					</div>
					<button
						type="button"
						aria-label="Toggle copy defanged"
						class="relative h-6 w-11 shrink-0 rounded-full transition-colors {contextMenuSettings
							.settings.showDefanged
							? 'bg-[var(--color-accent)]'
							: 'bg-[var(--color-bg-tertiary)]'}"
						onclick={() =>
							contextMenuSettings.update({
								showDefanged: !contextMenuSettings.settings.showDefanged
							})}
					>
						<span
							class="absolute top-0.5 h-5 w-5 rounded-full bg-white transition-[left] {contextMenuSettings
								.settings.showDefanged
								? 'left-[22px]'
								: 'left-0.5'}"
						></span>
					</button>
				</div>

				<div class="flex items-center justify-between py-3 first:pt-0 last:pb-0">
					<div>
						<div class="text-sm font-medium text-[var(--color-text-primary)]">
							Send to Workbench
						</div>
						<div class="mt-0.5 text-xs text-[var(--color-text-secondary)]">
							Open values in the transform workbench
						</div>
					</div>
					<button
						type="button"
						aria-label="Toggle send to workbench"
						class="relative h-6 w-11 shrink-0 rounded-full transition-colors {contextMenuSettings
							.settings.showSendToWorkbench
							? 'bg-[var(--color-accent)]'
							: 'bg-[var(--color-bg-tertiary)]'}"
						onclick={() =>
							contextMenuSettings.update({
								showSendToWorkbench: !contextMenuSettings.settings.showSendToWorkbench
							})}
					>
						<span
							class="absolute top-0.5 h-5 w-5 rounded-full bg-white transition-[left] {contextMenuSettings
								.settings.showSendToWorkbench
								? 'left-[22px]'
								: 'left-0.5'}"
						></span>
					</button>
				</div>

				<div class="flex items-center justify-between py-3 first:pt-0 last:pb-0">
					<div>
						<div class="text-sm font-medium text-[var(--color-text-primary)]">
							Transform submenu
						</div>
						<div class="mt-0.5 text-xs text-[var(--color-text-secondary)]">
							Show registered transforms as a submenu
						</div>
					</div>
					<button
						type="button"
						aria-label="Toggle transform submenu"
						class="relative h-6 w-11 shrink-0 rounded-full transition-colors {contextMenuSettings
							.settings.showTransformSubmenu
							? 'bg-[var(--color-accent)]'
							: 'bg-[var(--color-bg-tertiary)]'}"
						onclick={() =>
							contextMenuSettings.update({
								showTransformSubmenu: !contextMenuSettings.settings.showTransformSubmenu
							})}
					>
						<span
							class="absolute top-0.5 h-5 w-5 rounded-full bg-white transition-[left] {contextMenuSettings
								.settings.showTransformSubmenu
								? 'left-[22px]'
								: 'left-0.5'}"
						></span>
					</button>
				</div>
			</div>

			<!-- Custom actions list -->
			{#if contextMenuSettings.settings.customActions.length > 0}
				<div class="mt-5 border-t border-[var(--color-border)]/20 pt-5">
					<div
						class="mb-3 text-[10px] font-medium uppercase tracking-wider text-[var(--color-text-secondary)]"
					>
						Custom
					</div>
					<div class="space-y-3">
						{#each contextMenuSettings.settings.customActions as action (action.id)}
							<div
								class="flex items-center justify-between rounded-xl bg-[var(--color-bg-tertiary)] px-4 py-3 {action.enabled
									? ''
									: 'opacity-50'}"
							>
								<div class="min-w-0 flex-1">
									<div class="flex items-center gap-2">
										<span class="text-sm font-medium text-[var(--color-text-primary)]"
											>{action.name}</span
										>
										{#if !action.enabled}
											<span
												class="rounded bg-[var(--color-bg-card)] px-1.5 py-0.5 text-[10px] text-[var(--color-text-secondary)]"
											>
												Off
											</span>
										{/if}
									</div>
									<div class="mt-1 flex items-center gap-2">
										{#if action.actionType === 'open-url' && action.urlTemplate}
											<span
												class="truncate font-mono text-xs text-[var(--color-text-secondary)]"
												title={action.urlTemplate}>{action.urlTemplate}</span
											>
										{:else if action.actionType === 'run-transform' && action.transformId}
											<span class="text-xs text-[var(--color-text-secondary)]"
												>Transform: <span class="font-mono text-[var(--color-accent)]"
													>{action.transformId}</span
												></span
											>
										{:else if action.actionType === 'run-recipe'}
											<span class="text-xs text-[var(--color-text-secondary)]">Recipe</span>
										{:else if action.actionType === 'send-to-workbench'}
											<span class="text-xs text-[var(--color-text-secondary)]"
												>Workbench{action.transformId ? ` + ${action.transformId}` : ''}</span
											>
										{:else}
											<span class="text-xs text-[var(--color-text-secondary)]">Copy</span>
										{/if}
										<span class="text-[var(--color-border)]">&middot;</span>
										{#each action.contextTypes as t (t)}
											<span class="text-[10px] text-[var(--color-text-secondary)]">{t}</span>
										{/each}
									</div>
								</div>
								<div class="flex items-center gap-1">
									<button
										type="button"
										class="rounded-lg px-2.5 py-1 text-xs text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-card)] hover:text-[var(--color-text-primary)]"
										onclick={() =>
											contextMenuSettings.updateAction(action.id, { enabled: !action.enabled })}
									>
										{action.enabled ? 'Disable' : 'Enable'}
									</button>
									<button
										type="button"
										class="rounded-lg px-2.5 py-1 text-xs text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-card)] hover:text-[var(--color-text-primary)]"
										onclick={() => openEditAction(action)}
									>
										Edit
									</button>
									<button
										type="button"
										class="rounded-lg px-2.5 py-1 text-xs text-red-400 transition-colors hover:bg-red-500/10"
										onclick={() => contextMenuSettings.removeAction(action.id)}
									>
										Delete
									</button>
								</div>
							</div>
						{/each}
					</div>
				</div>
			{/if}
		</div>

		<!-- Defang Format -->
		<div>
			<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">Defang Format</h2>
			<p class="mt-0.5 text-sm text-[var(--color-text-secondary)]">
				Configure how indicators are defanged when copied.
			</p>
		</div>
		<div class="rounded-2xl bg-[var(--color-bg-secondary)] p-5">
			<div class="grid grid-cols-2 gap-4">
				<div>
					<label
						for="defang-dot"
						class="mb-1.5 block text-xs font-medium text-[var(--color-text-secondary)]"
						>Dot replacement</label
					>
					<select
						id="defang-dot"
						class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2.5 font-mono text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
						value={contextMenuSettings.settings.dotReplacement}
						onchange={(e) =>
							contextMenuSettings.update({
								dotReplacement: (e.currentTarget as HTMLSelectElement).value as
									| '[.]'
									| '(.)'
									| '{.}'
							})}
					>
						{#each dotOptions as opt (opt.value)}
							<option value={opt.value}>{opt.label}</option>
						{/each}
					</select>
				</div>
				<div>
					<label
						for="defang-protocol"
						class="mb-1.5 block text-xs font-medium text-[var(--color-text-secondary)]"
						>Protocol replacement</label
					>
					<select
						id="defang-protocol"
						class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2.5 font-mono text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
						value={contextMenuSettings.settings.protocolReplacement}
						onchange={(e) =>
							contextMenuSettings.update({
								protocolReplacement: (e.currentTarget as HTMLSelectElement).value as 'hxxp' | 'hXXp'
							})}
					>
						{#each protocolOptions as opt (opt.value)}
							<option value={opt.value}>{opt.label}</option>
						{/each}
					</select>
				</div>
			</div>

			<!-- Preview -->
			<div class="mt-4 rounded-lg bg-[var(--color-bg-tertiary)] p-3">
				<div
					class="mb-1 text-[10px] font-medium uppercase tracking-wider text-[var(--color-text-secondary)]"
				>
					Preview
				</div>
				<div class="font-mono text-sm">
					<span class="text-[var(--color-text-secondary)] line-through"
						>https://malware.example.com</span
					>
					<span class="mx-2 text-[var(--color-accent)]">&rarr;</span>
					<span class="text-[var(--color-text-primary)]">{defangPreview}</span>
				</div>
			</div>

			<!-- Defangable types -->
			<div class="mt-4">
				<span class="mb-2 block text-xs font-medium text-[var(--color-text-secondary)]"
					>Indicator types that show defang option</span
				>
				<div class="flex flex-wrap gap-2">
					{#each allDefangTypes as t (t)}
						<button
							type="button"
							class="rounded-full px-3 py-1 text-xs font-medium transition-colors
								{contextMenuSettings.settings.defangTypes.includes(t)
								? 'bg-[var(--color-accent)] text-white'
								: 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
							onclick={() => toggleDefangType(t)}
						>
							{t}
						</button>
					{/each}
				</div>
			</div>
		</div>

		<!-- Action Add/Edit Modal -->
		{#if showActionForm}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<!-- svelte-ignore a11y_click_events_have_key_events -->
			<div
				class="fixed inset-0 z-40 flex items-center justify-center bg-black/50 backdrop-blur-sm"
				onclick={() => (showActionForm = false)}
			>
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<div
					class="w-full max-w-lg rounded-2xl bg-[var(--color-bg-secondary)] p-6 shadow-2xl"
					onclick={(e) => e.stopPropagation()}
				>
					<h2 class="mb-4 text-lg font-semibold text-[var(--color-text-primary)]">
						{actionEditing ? 'Edit Action' : 'Add Custom Action'}
					</h2>

					<div class="space-y-4">
						<div>
							<label
								for="action-form-name"
								class="mb-1 block text-xs font-medium text-[var(--color-text-secondary)]"
								>Name</label
							>
							<input
								id="action-form-name"
								type="text"
								bind:value={actionFormName}
								placeholder="e.g. Submit to Internal Sandbox"
								class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2.5 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
							/>
						</div>

						<div>
							<label
								for="action-form-type"
								class="mb-1 block text-xs font-medium text-[var(--color-text-secondary)]"
								>Action type</label
							>
							<select
								id="action-form-type"
								class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2.5 text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
								value={actionFormType}
								onchange={(e) =>
									onActionTypeChange(
										(e.currentTarget as HTMLSelectElement).value as CustomActionType
									)}
							>
								{#each actionTypeOptions as opt (opt.value)}
									<option value={opt.value}>{opt.label}</option>
								{/each}
							</select>
						</div>

						{#if actionFormType === 'open-url'}
							<div>
								<label
									for="action-form-url"
									class="mb-1 block text-xs font-medium text-[var(--color-text-secondary)]"
								>
									URL template
									<span class="font-normal opacity-70">- use {'{value}'} as placeholder</span>
								</label>
								<input
									id="action-form-url"
									type="text"
									bind:value={actionFormUrl}
									placeholder="https://example.com/submit?q={'{value}'}"
									class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2.5 font-mono text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
								/>
							</div>
						{:else if actionFormType === 'run-transform' || actionFormType === 'send-to-workbench'}
							<div>
								<label
									for="action-form-transform"
									class="mb-1 block text-xs font-medium text-[var(--color-text-secondary)]"
								>
									Transform
									{#if actionFormType === 'send-to-workbench'}
										<span class="font-normal opacity-70">- optional, pre-loads in workbench</span>
									{/if}
								</label>
								<select
									id="action-form-transform"
									class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2.5 text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
									bind:value={actionFormTransformId}
								>
									{#if actionFormType === 'send-to-workbench'}
										<option value="">None</option>
									{:else}
										<option value="" disabled>Select a transform</option>
									{/if}
									{#each transformStore.transforms as t (t.id)}
										<option value={t.id}>{t.name} ({t.category})</option>
									{/each}
								</select>
							</div>
						{:else if actionFormType === 'run-recipe'}
							<div>
								<label
									for="action-form-recipe"
									class="mb-1 block text-xs font-medium text-[var(--color-text-secondary)]"
									>Recipe</label
								>
								{#if !recipesLoaded}
									<p class="text-xs text-[var(--color-text-secondary)]">Loading recipes...</p>
								{:else if recipes.length === 0}
									<p class="text-xs text-[var(--color-text-secondary)]">
										No recipes available. Create recipes in Settings &rarr; Transforms & Recipes.
									</p>
								{:else}
									<select
										id="action-form-recipe"
										class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2.5 text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
										bind:value={actionFormRecipeId}
									>
										<option value="" disabled>Select a recipe</option>
										{#each recipes as r (r.id)}
											<option value={r.id}>{r.name}</option>
										{/each}
									</select>
								{/if}
							</div>
						{/if}

						<div>
							<span class="mb-2 block text-xs font-medium text-[var(--color-text-secondary)]"
								>Context types</span
							>
							<div class="flex flex-wrap gap-2">
								{#each allContextTypes as t (t)}
									<button
										type="button"
										class="rounded-full px-3 py-1 text-xs font-medium transition-colors
											{actionFormContextTypes.includes(t)
											? 'bg-[var(--color-accent)] text-white'
											: 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
										onclick={() => toggleContextType(t)}
									>
										{t}
									</button>
								{/each}
							</div>
						</div>

						<div class="flex items-center gap-2">
							<button
								type="button"
								class="size-5 rounded border transition-colors
									{actionFormEnabled
									? 'border-[var(--color-accent)] bg-[var(--color-accent)]'
									: 'border-[var(--color-border)] bg-[var(--color-bg-tertiary)]'}"
								onclick={() => (actionFormEnabled = !actionFormEnabled)}
							>
								{#if actionFormEnabled}
									<svg
										xmlns="http://www.w3.org/2000/svg"
										viewBox="0 0 20 20"
										fill="currentColor"
										class="size-5 text-white"
									>
										<path
											fill-rule="evenodd"
											d="M16.704 4.153a.75.75 0 0 1 .143 1.052l-8 10.5a.75.75 0 0 1-1.127.075l-4.5-4.5a.75.75 0 0 1 1.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 0 1 1.05-.143Z"
											clip-rule="evenodd"
										/>
									</svg>
								{/if}
							</button>
							<span class="text-sm text-[var(--color-text-primary)]">Enabled</span>
						</div>
					</div>

					<div class="mt-6 flex justify-end gap-3">
						<button
							type="button"
							class="rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2 text-sm text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text-primary)]"
							onclick={() => (showActionForm = false)}
						>
							Cancel
						</button>
						<button
							type="button"
							class="rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-[var(--color-accent-hover)] disabled:opacity-50"
							disabled={actionSaveDisabled}
							onclick={saveAction}
						>
							{actionEditing ? 'Save Changes' : 'Add Action'}
						</button>
					</div>
				</div>
			</div>
		{/if}
	{/if}
</div>
