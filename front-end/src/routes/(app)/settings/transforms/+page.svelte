<script lang="ts">
	import { invalidate } from '$app/navigation';
	import {
		createCustomTransform,
		deleteCustomTransform,
		updateCustomTransform
	} from '$lib/api/transforms';
	import { deleteRecipe } from '$lib/api/recipes';
	import type { TransformKind, CustomTransform, Recipe, RecipeScope } from '$lib/api/types';
	import type { PageData } from './$types';

	interface Props {
		data: PageData;
	}

	let { data }: Props = $props();

	// ── Tab state ──────────────────────────────────────────────
	let activeTab = $state<'transforms' | 'recipes'>('transforms');

	// ── Shared state ───────────────────────────────────────────
	let error = $state<string | null>(null);

	// ── Transform state ────────────────────────────────────────
	let showCreate = $state(false);
	let newTransformId = $state('');
	let newName = $state('');
	let newCategory = $state('custom');
	let newKind = $state<TransformKind>('yaml');
	let newContent = $state('');
	let creating = $state(false);

	let editingId = $state<string | null>(null);
	let editName = $state('');
	let editCategory = $state('');
	let editContent = $state('');
	let editEnabled = $state(true);
	let saving = $state(false);

	function startEdit(t: CustomTransform) {
		editingId = t.id;
		editName = t.name;
		editCategory = t.category;
		editContent = t.content;
		editEnabled = t.enabled;
	}

	function cancelEdit() {
		editingId = null;
	}

	async function handleSaveEdit() {
		if (!editingId || !editName.trim() || !editContent.trim()) return;
		saving = true;
		error = null;
		try {
			await updateCustomTransform(fetch, editingId, {
				name: editName.trim(),
				category: editCategory.trim(),
				content: editContent,
				enabled: editEnabled
			});
			editingId = null;
			await invalidate('malbox:transforms');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			saving = false;
		}
	}

	async function handleToggleEnabled(t: CustomTransform) {
		try {
			await updateCustomTransform(fetch, t.id, {
				name: t.name,
				category: t.category,
				content: t.content,
				enabled: !t.enabled
			});
			await invalidate('malbox:transforms');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function handleCreateTransform() {
		creating = true;
		error = null;
		try {
			await createCustomTransform(fetch, {
				transform_id: newTransformId,
				name: newName,
				category: newCategory,
				kind: newKind,
				content: newContent
			});
			showCreate = false;
			newTransformId = '';
			newName = '';
			newContent = '';
			await invalidate('malbox:transforms');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			creating = false;
		}
	}

	async function handleDeleteTransform(id: string) {
		try {
			await deleteCustomTransform(fetch, id);
			await invalidate('malbox:transforms');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	const kindBadgeColor: Record<TransformKind, string> = {
		yaml: 'bg-green-500/10 text-green-400',
		js: 'bg-yellow-500/10 text-yellow-400',
		wasm: 'bg-purple-500/10 text-purple-400'
	};

	// ── Recipe state ───────────────────────────────────────────
	let recipeSearch = $state('');
	let recipeFilter = $state<'all' | RecipeScope>('all');
	let deleteConfirm = $state<string | null>(null);

	const filteredRecipes = $derived(() => {
		let list = data.recipes;
		if (recipeFilter !== 'all') {
			list = list.filter((r) => r.scope === recipeFilter);
		}
		const q = recipeSearch.trim().toLowerCase();
		if (q) {
			list = list.filter(
				(r) =>
					r.name.toLowerCase().includes(q) ||
					r.description?.toLowerCase().includes(q) ||
					r.tags.some((t) => t.toLowerCase().includes(q)) ||
					r.steps.some((s) => s.transform_id.toLowerCase().includes(q))
			);
		}
		return list;
	});

	async function handleDeleteRecipe(id: string) {
		try {
			await deleteRecipe(fetch, id);
			deleteConfirm = null;
			await invalidate('malbox:recipes');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	function formatDate(iso: string): string {
		return new Date(iso).toLocaleDateString('en-US', {
			month: 'short',
			day: 'numeric',
			year: 'numeric'
		});
	}

	function openInWorkbench(recipe: Recipe) {
		const stepsParam = encodeURIComponent(
			JSON.stringify(
				recipe.steps.map((s) => ({
					transformId: s.transform_id,
					params: s.params,
					source: 'manual'
				}))
			)
		);
		window.location.href = `/workbench?steps=${stepsParam}`;
	}

	const scopeBadge: Record<RecipeScope, string> = {
		personal: 'bg-blue-500/10 text-blue-400',
		shared: 'bg-emerald-400/10 text-emerald-400'
	};
</script>

<div class="mx-auto max-w-7xl space-y-6">
	<div class="flex items-center gap-3 text-sm text-[var(--color-text-secondary)]">
		<a href="/settings" class="hover:text-[var(--color-text-primary)]">Settings</a>
		<span>/</span>
		<span class="text-[var(--color-text-primary)]">Transforms &amp; Recipes</span>
	</div>

	{#if error}
		<div class="rounded-lg border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-300">
			{error}
		</div>
	{/if}

	<div class="flex items-center justify-between gap-4">
		<h1 class="text-3xl font-semibold text-[var(--color-text-primary)]">
			Transforms &amp; Recipes
		</h1>

		<div class="flex items-center gap-3">
			<div class="flex items-center rounded-xl bg-[var(--color-bg-tertiary)] p-1.5">
				<button
					onclick={() => (activeTab = 'transforms')}
					class="rounded-lg px-5 py-2.5 text-sm font-medium transition-colors {activeTab ===
					'transforms'
						? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
						: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				>
					Transforms
					{#if data.transforms.length > 0}
						<span class="ml-1 text-xs opacity-60">{data.transforms.length}</span>
					{/if}
				</button>
				<button
					onclick={() => (activeTab = 'recipes')}
					class="rounded-lg px-5 py-2.5 text-sm font-medium transition-colors {activeTab ===
					'recipes'
						? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
						: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				>
					Recipes
					{#if data.recipes.length > 0}
						<span class="ml-1 text-xs opacity-60">{data.recipes.length}</span>
					{/if}
				</button>
			</div>

			{#if activeTab === 'transforms'}
				<button
					onclick={() => (showCreate = !showCreate)}
					class="rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white transition-colors hover:opacity-90"
				>
					{showCreate ? 'Cancel' : 'Add transform'}
				</button>
			{:else}
				<a
					href="/workbench"
					class="rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white transition-colors hover:opacity-90"
				>
					Create in Workbench
				</a>
			{/if}
		</div>
	</div>

	<!-- ── Create transform form ─────────────────────────────── -->
	{#if activeTab === 'transforms' && showCreate}
		<div
			class="space-y-4 rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-6"
		>
			<div class="grid grid-cols-1 gap-4 md:grid-cols-2">
				<div>
					<label for="tf-id" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
						>Transform ID</label
					>
					<input
						id="tf-id"
						bind:value={newTransformId}
						placeholder="my-custom-xor"
						class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
					/>
				</div>
				<div>
					<label for="tf-name" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
						>Name</label
					>
					<input
						id="tf-name"
						bind:value={newName}
						placeholder="My Custom XOR"
						class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
					/>
				</div>
				<div>
					<label for="tf-cat" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
						>Category</label
					>
					<input
						id="tf-cat"
						bind:value={newCategory}
						placeholder="custom"
						class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
					/>
				</div>
				<div>
					<label for="tf-kind" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
						>Kind</label
					>
					<select
						id="tf-kind"
						bind:value={newKind}
						class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
					>
						<option value="yaml">YAML</option>
						<option value="js">JavaScript</option>
						<option value="wasm">WASM</option>
					</select>
				</div>
			</div>
			<div>
				<label for="tf-content" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
					>Content</label
				>
				<textarea
					id="tf-content"
					bind:value={newContent}
					rows={8}
					placeholder={newKind === 'yaml'
						? 'id: my-transform\nname: My Transform\ncategory: custom\ntemplate: xor\nparams:\n  key: "0x41"'
						: 'export default { ... }'}
					class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 font-mono text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
				></textarea>
			</div>
			<button
				onclick={handleCreateTransform}
				disabled={creating || !newTransformId || !newName || !newContent}
				class="rounded-lg bg-[var(--color-accent)] px-6 py-2 text-sm font-medium text-white transition-colors hover:bg-[var(--color-accent-hover)] disabled:opacity-50"
			>
				{creating ? 'Creating...' : 'Create'}
			</button>
		</div>
	{/if}

	<!-- ── Transforms content ────────────────────────────────── -->
	{#if activeTab === 'transforms'}
		<div class="space-y-6 rounded-2xl bg-[var(--color-bg-secondary)] p-6">
			{#if data.transforms.length === 0 && !showCreate}
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
								d="M8.34 1.804A1 1 0 0 1 9.32 1h1.36a1 1 0 0 1 .98.804l.295 1.473c.497.144.971.342 1.416.587l1.25-.834a1 1 0 0 1 1.262.125l.962.962a1 1 0 0 1 .125 1.262l-.834 1.25c.245.445.443.919.587 1.416l1.473.295a1 1 0 0 1 .804.98v1.36a1 1 0 0 1-.804.98l-1.473.295a6.95 6.95 0 0 1-.587 1.416l.834 1.25a1 1 0 0 1-.125 1.262l-.962.962a1 1 0 0 1-1.262.125l-1.25-.834a6.953 6.953 0 0 1-1.416.587l-.295 1.473a1 1 0 0 1-.98.804H9.32a1 1 0 0 1-.98-.804l-.295-1.473a6.957 6.957 0 0 1-1.416-.587l-1.25.834a1 1 0 0 1-1.262-.125l-.962-.962a1 1 0 0 1-.125-1.262l.834-1.25a6.957 6.957 0 0 1-.587-1.416l-1.473-.295A1 1 0 0 1 1 11.36V10a1 1 0 0 1 .804-.98l1.473-.295c.144-.497.342-.971.587-1.416l-.834-1.25a1 1 0 0 1 .125-1.262l.962-.962A1 1 0 0 1 5.38 3.71l1.25.834a6.957 6.957 0 0 1 1.416-.587l.294-1.473ZM13 10a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z"
								clip-rule="evenodd"
							/>
						</svg>
					</div>
					<p class="text-sm text-[var(--color-text-secondary)]">
						No custom transforms yet. Click "Add transform" to create one.
					</p>
				</div>
			{:else}
				<div class="overflow-hidden rounded-lg border border-[var(--color-border)]">
					<div
						class="grid grid-cols-[1fr_140px_80px_70px_60px_120px] gap-4 border-b border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-4 py-2 text-xs font-medium text-[var(--color-text-secondary)]"
					>
						<div>Name</div>
						<div>Transform ID</div>
						<div>Kind</div>
						<div>Source</div>
						<div>Active</div>
						<div></div>
					</div>
					{#each data.transforms as t (t.id)}
						<div class="border-b border-[var(--color-border)] last:border-b-0">
							<div
								class="grid grid-cols-[1fr_140px_80px_70px_60px_120px] items-center gap-4 px-4 py-3 text-sm"
							>
								<div>
									<div class="text-[var(--color-text-primary)]">{t.name}</div>
									<div class="text-xs text-[var(--color-text-secondary)]">{t.category}</div>
								</div>
								<div class="font-mono text-xs text-[var(--color-text-secondary)]">
									{t.transform_id}
								</div>
								<div>
									<span class="rounded px-2 py-0.5 text-xs font-medium {kindBadgeColor[t.kind]}"
										>{t.kind}</span
									>
								</div>
								<div class="text-xs text-[var(--color-text-secondary)]">
									{t.git_synced ? 'git' : 'ui'}
								</div>
								<div>
									{#if !t.git_synced}
										<button
											onclick={() => handleToggleEnabled(t)}
											class="rounded px-2 py-0.5 text-xs font-medium transition-colors {t.enabled
												? 'bg-emerald-400/10 text-emerald-400'
												: 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)]'}"
										>
											{t.enabled ? 'on' : 'off'}
										</button>
									{:else}
										<span
											class="rounded px-2 py-0.5 text-xs {t.enabled
												? 'text-emerald-400'
												: 'text-[var(--color-text-secondary)]'}"
										>
											{t.enabled ? 'on' : 'off'}
										</span>
									{/if}
								</div>
								<div class="flex items-center justify-end gap-1">
									{#if !t.git_synced}
										<button
											onclick={() => startEdit(t)}
											class="rounded-lg px-2 py-1 text-xs text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-primary)]"
										>
											Edit
										</button>
										<button
											onclick={() => handleDeleteTransform(t.id)}
											class="rounded-lg px-2 py-1 text-xs text-red-400 transition-colors hover:bg-red-400/10 hover:text-red-300"
										>
											Delete
										</button>
									{/if}
								</div>
							</div>

							{#if editingId === t.id}
								<div
									class="space-y-4 border-t border-[var(--color-border)] bg-[var(--color-bg-tertiary)]/30 px-6 py-5"
								>
									<div class="grid grid-cols-1 gap-4 md:grid-cols-2">
										<div>
											<label
												for="edit-name-{t.id}"
												class="mb-1 block text-xs text-[var(--color-text-secondary)]">Name</label
											>
											<input
												id="edit-name-{t.id}"
												bind:value={editName}
												class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
											/>
										</div>
										<div>
											<label
												for="edit-cat-{t.id}"
												class="mb-1 block text-xs text-[var(--color-text-secondary)]"
												>Category</label
											>
											<input
												id="edit-cat-{t.id}"
												bind:value={editCategory}
												class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
											/>
										</div>
									</div>
									<div>
										<label
											for="edit-content-{t.id}"
											class="mb-1 block text-xs text-[var(--color-text-secondary)]">Content</label
										>
										<textarea
											id="edit-content-{t.id}"
											bind:value={editContent}
											rows={6}
											class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 font-mono text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
										></textarea>
									</div>
									<div class="flex items-center justify-between">
										<label
											class="flex cursor-pointer items-center gap-2 text-sm text-[var(--color-text-secondary)]"
										>
											<input
												type="checkbox"
												bind:checked={editEnabled}
												class="rounded border-[var(--color-border)]"
											/>
											Enabled
										</label>
										<div class="flex items-center gap-2">
											<button
												onclick={cancelEdit}
												class="rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2 text-sm text-[var(--color-text-secondary)]"
											>
												Cancel
											</button>
											<button
												onclick={handleSaveEdit}
												disabled={saving || !editName.trim() || !editContent.trim()}
												class="rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white transition-colors hover:opacity-90 disabled:opacity-50"
											>
												{saving ? 'Saving...' : 'Save'}
											</button>
										</div>
									</div>
								</div>
							{/if}
						</div>
					{/each}
				</div>
			{/if}
		</div>
	{/if}

	<!-- ── Recipes content ───────────────────────────────────── -->
	{#if activeTab === 'recipes'}
		<div class="space-y-6 rounded-2xl bg-[var(--color-bg-secondary)] p-6">
			<div class="flex items-center gap-4">
				<div class="relative max-w-md flex-1">
					<input
						type="text"
						placeholder="Search recipes"
						bind:value={recipeSearch}
						class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
					/>
				</div>
				<div class="flex items-center rounded-lg bg-[var(--color-bg-tertiary)] p-1">
					{#each ['all', 'personal', 'shared'] as scope (scope)}
						<button
							onclick={() => (recipeFilter = scope as 'all' | RecipeScope)}
							class="rounded-md px-3 py-1.5 text-xs font-medium transition-colors {recipeFilter ===
							scope
								? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
								: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
						>
							{scope.charAt(0).toUpperCase() + scope.slice(1)}
						</button>
					{/each}
				</div>
			</div>

			{#if filteredRecipes().length === 0}
				<div class="py-12 text-center">
					<div
						class="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-xl bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)]"
					>
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 24 24"
							fill="currentColor"
							class="size-6"
						>
							<path
								fill-rule="evenodd"
								d="M5.625 1.5c-1.036 0-1.875.84-1.875 1.875v17.25c0 1.035.84 1.875 1.875 1.875h12.75c1.035 0 1.875-.84 1.875-1.875V12.75A3.75 3.75 0 0 0 16.5 9h-1.875a1.875 1.875 0 0 1-1.875-1.875V5.25A3.75 3.75 0 0 0 9 1.5H5.625ZM7.5 15a.75.75 0 0 1 .75-.75h3a.75.75 0 0 1 0 1.5h-3A.75.75 0 0 1 7.5 15Zm.75 2.25a.75.75 0 0 0 0 1.5h6a.75.75 0 0 0 0-1.5h-6Z"
								clip-rule="evenodd"
							/>
							<path
								d="M12.971 1.816A5.23 5.23 0 0 1 14.25 5.25v1.875c0 .207.168.375.375.375H16.5a5.23 5.23 0 0 1 3.434 1.279 9.768 9.768 0 0 0-6.963-6.963Z"
							/>
						</svg>
					</div>
					<p class="text-sm text-[var(--color-text-secondary)]">
						{data.recipes.length === 0
							? 'No recipes yet. Create one in the Workbench.'
							: 'No recipes match the current filter.'}
					</p>
				</div>
			{:else}
				<div class="overflow-hidden rounded-lg border border-[var(--color-border)]">
					<div
						class="grid grid-cols-[1fr_100px_1fr_120px_80px] gap-4 border-b border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-4 py-2 text-xs font-medium text-[var(--color-text-secondary)]"
					>
						<div>Name</div>
						<div>Scope</div>
						<div>Steps</div>
						<div>Created</div>
						<div></div>
					</div>

					{#each filteredRecipes() as recipe (recipe.id)}
						<div
							class="grid grid-cols-[1fr_100px_1fr_120px_80px] items-center gap-4 border-b border-[var(--color-border)] px-4 py-3 text-sm last:border-b-0"
						>
							<div>
								<a
									href="/settings/transforms/recipes/{recipe.id}"
									class="font-medium text-[var(--color-text-primary)] transition-colors hover:text-[var(--color-accent)]"
								>
									{recipe.name}
								</a>
								{#if recipe.description}
									<div
										class="mt-0.5 truncate text-xs text-[var(--color-text-secondary)]"
										title={recipe.description}
									>
										{recipe.description}
									</div>
								{/if}
								{#if recipe.tags.length > 0}
									<div class="mt-1 flex flex-wrap gap-1">
										{#each recipe.tags as tag (tag)}
											<span
												class="rounded bg-[var(--color-bg-tertiary)] px-1.5 py-0.5 text-[10px] text-[var(--color-text-secondary)]"
											>
												{tag}
											</span>
										{/each}
									</div>
								{/if}
							</div>
							<div>
								<span class="rounded px-2 py-0.5 text-xs font-medium {scopeBadge[recipe.scope]}">
									{recipe.scope}
								</span>
							</div>
							<div>
								<div class="flex items-center gap-1 overflow-hidden">
									{#each recipe.steps as step, i (i)}
										{#if i > 0}
											<svg
												class="h-3 w-3 shrink-0 text-[var(--color-text-secondary)]/50"
												viewBox="0 0 20 20"
												fill="currentColor"
											>
												<path
													fill-rule="evenodd"
													d="M7.21 14.77a.75.75 0 0 1 .02-1.06L11.168 10 7.23 6.29a.75.75 0 1 1 1.04-1.08l4.5 4.25a.75.75 0 0 1 0 1.08l-4.5 4.25a.75.75 0 0 1-1.06-.02Z"
													clip-rule="evenodd"
												/>
											</svg>
										{/if}
										<span
											class="shrink-0 truncate rounded bg-[var(--color-bg-tertiary)] px-1.5 py-0.5 font-mono text-[10px] text-[var(--color-text-secondary)]"
										>
											{step.transform_id}
										</span>
									{/each}
								</div>
							</div>
							<div class="text-xs text-[var(--color-text-secondary)]">
								{formatDate(recipe.created_on)}
							</div>
							<div class="flex items-center justify-end gap-1">
								<button
									onclick={() => openInWorkbench(recipe)}
									class="rounded-lg px-2 py-1 text-xs text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-primary)]"
									title="Load in Workbench"
								>
									Run
								</button>
								{#if deleteConfirm === recipe.id}
									<button
										onclick={() => handleDeleteRecipe(recipe.id)}
										class="rounded-lg px-2 py-1 text-xs font-medium text-red-400 transition-colors hover:bg-red-400/10"
									>
										Confirm
									</button>
									<button
										onclick={() => (deleteConfirm = null)}
										class="rounded-lg px-2 py-1 text-xs text-[var(--color-text-secondary)]"
									>
										No
									</button>
								{:else}
									<button
										onclick={() => (deleteConfirm = recipe.id)}
										class="rounded-lg px-2 py-1 text-xs text-red-400 transition-colors hover:bg-red-400/10 hover:text-red-300"
									>
										Delete
									</button>
								{/if}
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	{/if}
</div>
