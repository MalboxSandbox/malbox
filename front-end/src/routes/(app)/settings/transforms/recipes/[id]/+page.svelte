<script lang="ts">
	import { goto, invalidate } from '$app/navigation';
	import { updateRecipe, deleteRecipe } from '$lib/api/recipes';
	import { transformStore } from '$lib/stores/transforms.svelte';
	import type { RecipeScope, RecipeStep } from '$lib/api/types';
	import type { PageData } from './$types';

	interface Props {
		data: PageData;
	}

	let { data }: Props = $props();

	let name = $state(data.recipe.name);
	let description = $state(data.recipe.description ?? '');
	let scope = $state<RecipeScope>(data.recipe.scope);
	let tagsInput = $state(data.recipe.tags.join(', '));
	let steps = $state<RecipeStep[]>(structuredClone(data.recipe.steps));

	let saving = $state(false);
	let error = $state<string | null>(null);
	let deleteConfirm = $state(false);
	let dirty = $state(false);

	$effect(() => {
		const nameChanged = name !== data.recipe.name;
		const descChanged = (description || null) !== (data.recipe.description || null);
		const scopeChanged = scope !== data.recipe.scope;
		const tagsChanged = tagsInput !== data.recipe.tags.join(', ');
		const stepsChanged = JSON.stringify(steps) !== JSON.stringify(data.recipe.steps);
		dirty = nameChanged || descChanged || scopeChanged || tagsChanged || stepsChanged;
	});

	async function handleSave() {
		if (!name.trim()) return;
		saving = true;
		error = null;
		try {
			await updateRecipe(fetch, data.recipe.id, {
				name: name.trim(),
				description: description.trim() || undefined,
				scope,
				tags: tagsInput
					.split(',')
					.map((t) => t.trim())
					.filter(Boolean),
				steps
			});
			await invalidate('malbox:recipes');
			dirty = false;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			saving = false;
		}
	}

	async function handleDelete() {
		try {
			await deleteRecipe(fetch, data.recipe.id);
			await goto('/settings/transforms');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	function removeStep(index: number) {
		steps = steps.filter((_, i) => i !== index);
	}

	function moveStep(from: number, to: number) {
		if (to < 0 || to >= steps.length) return;
		const newSteps = [...steps];
		const [moved] = newSteps.splice(from, 1);
		newSteps.splice(to, 0, moved);
		steps = newSteps;
	}

	function openInWorkbench() {
		const stepsParam = encodeURIComponent(
			JSON.stringify(
				steps.map((s) => ({
					transformId: s.transform_id,
					params: s.params,
					source: 'manual'
				}))
			)
		);
		window.location.href = `/workbench?steps=${stepsParam}`;
	}

	function formatDate(iso: string): string {
		return new Date(iso).toLocaleDateString('en-US', {
			month: 'short',
			day: 'numeric',
			year: 'numeric',
			hour: '2-digit',
			minute: '2-digit'
		});
	}

	function resolveTransformName(id: string): string {
		return transformStore.registry.get(id)?.name ?? id;
	}

	function removeStepParam(stepIndex: number, key: string) {
		steps = steps.map((s, i) => {
			if (i !== stepIndex) return s;
			const { [key]: _, ...rest } = s.params;
			return { ...s, params: rest };
		});
	}

	const scopeBadge: Record<RecipeScope, string> = {
		personal: 'bg-blue-500/10 text-blue-400',
		shared: 'bg-emerald-400/10 text-emerald-400'
	};
</script>

<div class="mx-auto max-w-5xl space-y-6">
	<!-- Breadcrumb -->
	<div class="flex items-center gap-3 text-sm text-[var(--color-text-secondary)]">
		<a href="/settings" class="hover:text-[var(--color-text-primary)]">Settings</a>
		<span>/</span>
		<a href="/settings/transforms" class="hover:text-[var(--color-text-primary)]"
			>Transforms &amp; Recipes</a
		>
		<span>/</span>
		<span class="text-[var(--color-text-primary)]">{data.recipe.name}</span>
	</div>

	{#if error}
		<div class="rounded-lg border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-300">
			{error}
		</div>
	{/if}

	<!-- Header card -->
	<div class="rounded-2xl bg-[var(--color-bg-secondary)] p-8">
		<div class="flex items-start justify-between gap-4">
			<div>
				<h1 class="text-2xl font-semibold text-[var(--color-text-primary)]">
					{data.recipe.name}
				</h1>
				{#if data.recipe.description}
					<p class="mt-1 text-sm text-[var(--color-text-secondary)]">
						{data.recipe.description}
					</p>
				{/if}
			</div>
			<div class="flex items-center gap-2">
				<span class="rounded-lg px-3 py-1.5 text-sm font-medium {scopeBadge[data.recipe.scope]}">
					{data.recipe.scope}
				</span>
			</div>
		</div>

		<div class="mt-6 grid grid-cols-2 gap-x-8 gap-y-4 text-sm md:grid-cols-4">
			<div>
				<dt class="text-xs font-medium text-[var(--color-text-secondary)]">Author</dt>
				<dd class="mt-0.5 text-[var(--color-text-primary)]">{data.recipe.author}</dd>
			</div>
			<div>
				<dt class="text-xs font-medium text-[var(--color-text-secondary)]">Steps</dt>
				<dd class="mt-0.5 text-[var(--color-text-primary)]">{data.recipe.steps.length}</dd>
			</div>
			<div>
				<dt class="text-xs font-medium text-[var(--color-text-secondary)]">Created</dt>
				<dd class="mt-0.5 text-[var(--color-text-primary)]">
					{formatDate(data.recipe.created_on)}
				</dd>
			</div>
			<div>
				<dt class="text-xs font-medium text-[var(--color-text-secondary)]">Updated</dt>
				<dd class="mt-0.5 text-[var(--color-text-primary)]">
					{data.recipe.updated_at ? formatDate(data.recipe.updated_at) : '-'}
				</dd>
			</div>
		</div>

		{#if data.recipe.tags.length > 0}
			<div class="mt-4 flex flex-wrap gap-1.5 border-t border-[var(--color-border)] pt-4">
				{#each data.recipe.tags as tag (tag)}
					<span
						class="rounded bg-[var(--color-bg-tertiary)] px-2 py-0.5 text-xs text-[var(--color-text-secondary)]"
					>
						{tag}
					</span>
				{/each}
			</div>
		{/if}
	</div>

	<!-- Edit details -->
	<div class="rounded-2xl bg-[var(--color-bg-secondary)] p-8">
		<div class="flex items-center justify-between">
			<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">Edit Details</h2>
			<div class="flex items-center gap-2">
				<button
					onclick={openInWorkbench}
					class="rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2 text-sm font-medium text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text-primary)]"
				>
					Open in Workbench
				</button>
				{#if dirty}
					<button
						onclick={handleSave}
						disabled={saving || !name.trim()}
						class="rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white transition-colors hover:opacity-90 disabled:opacity-50"
					>
						{saving ? 'Saving...' : 'Save changes'}
					</button>
				{/if}
			</div>
		</div>

		<div class="mt-6 grid grid-cols-1 gap-4 md:grid-cols-2">
			<div>
				<label for="recipe-name" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
					>Name</label
				>
				<input
					id="recipe-name"
					bind:value={name}
					class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
				/>
			</div>
			<div>
				<label for="recipe-scope" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
					>Scope</label
				>
				<select
					id="recipe-scope"
					bind:value={scope}
					class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
				>
					<option value="personal">Personal</option>
					<option value="shared">Shared</option>
				</select>
			</div>
			<div class="md:col-span-2">
				<label for="recipe-desc" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
					>Description</label
				>
				<input
					id="recipe-desc"
					bind:value={description}
					placeholder="Optional description"
					class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
				/>
			</div>
			<div class="md:col-span-2">
				<label for="recipe-tags" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
					>Tags</label
				>
				<input
					id="recipe-tags"
					bind:value={tagsInput}
					placeholder="Comma-separated tags"
					class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
				/>
			</div>
		</div>
	</div>

	<!-- Pipeline steps -->
	<div class="rounded-2xl bg-[var(--color-bg-secondary)] p-8">
		<div class="flex items-center justify-between">
			<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">Pipeline Steps</h2>
			<span class="text-xs text-[var(--color-text-secondary)]">{steps.length} total</span>
		</div>

		{#if steps.length === 0}
			<p class="mt-4 text-sm text-[var(--color-text-secondary)]">
				No steps. Open in the Workbench to build a pipeline.
			</p>
		{:else}
			<div class="mt-4 space-y-2">
				{#each steps as step, i (i)}
					<div
						class="flex items-center justify-between rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3"
					>
						<div class="flex items-center gap-3">
							<div class="flex items-center gap-1">
								<button
									onclick={() => moveStep(i, i - 1)}
									disabled={i === 0}
									class="rounded p-0.5 text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text-primary)] disabled:opacity-20"
									title="Move up"
								>
									<svg class="h-3.5 w-3.5" viewBox="0 0 20 20" fill="currentColor">
										<path
											fill-rule="evenodd"
											d="M9.47 6.47a.75.75 0 0 1 1.06 0l4.25 4.25a.75.75 0 1 1-1.06 1.06L10 8.06l-3.72 3.72a.75.75 0 0 1-1.06-1.06l4.25-4.25Z"
											clip-rule="evenodd"
										/>
									</svg>
								</button>
								<button
									onclick={() => moveStep(i, i + 1)}
									disabled={i === steps.length - 1}
									class="rounded p-0.5 text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text-primary)] disabled:opacity-20"
									title="Move down"
								>
									<svg class="h-3.5 w-3.5" viewBox="0 0 20 20" fill="currentColor">
										<path
											fill-rule="evenodd"
											d="M5.22 8.22a.75.75 0 0 1 1.06 0L10 11.94l3.72-3.72a.75.75 0 1 1 1.06 1.06l-4.25 4.25a.75.75 0 0 1-1.06 0L5.22 9.28a.75.75 0 0 1 0-1.06Z"
											clip-rule="evenodd"
										/>
									</svg>
								</button>
							</div>
							<span
								class="flex h-6 w-6 items-center justify-center rounded bg-[var(--color-bg-secondary)] text-xs font-medium text-[var(--color-text-secondary)]"
							>
								{i + 1}
							</span>
							<div>
								<span class="text-sm text-[var(--color-text-primary)]">
									{resolveTransformName(step.transform_id)}
								</span>
								<span class="ml-1 font-mono text-xs text-[var(--color-text-secondary)]">
									{step.transform_id}
								</span>
							</div>
						</div>
						<div class="flex items-center gap-2">
							{#each Object.entries(step.params) as [key, value] (key)}
								<div class="flex items-center gap-1">
									<span
										class="rounded bg-[var(--color-bg-secondary)] px-1.5 py-0.5 font-mono text-[10px] text-[var(--color-text-secondary)]"
									>
										{key}={typeof value === 'string' ? `"${value}"` : value}
									</span>
									<button
										onclick={() => removeStepParam(i, key)}
										class="rounded p-0.5 text-[var(--color-text-secondary)] hover:text-red-400"
										title="Remove parameter"
									>
										<svg class="h-3 w-3" viewBox="0 0 20 20" fill="currentColor">
											<path
												d="M6.28 5.22a.75.75 0 00-1.06 1.06L8.94 10l-3.72 3.72a.75.75 0 101.06 1.06L10 11.06l3.72 3.72a.75.75 0 101.06-1.06L11.06 10l3.72-3.72a.75.75 0 00-1.06-1.06L10 8.94 6.28 5.22z"
											/>
										</svg>
									</button>
								</div>
							{/each}
							<button
								onclick={() => removeStep(i)}
								class="rounded-lg px-3 py-1 text-xs text-red-400 transition-colors hover:bg-red-400/10 hover:text-red-300"
							>
								Delete
							</button>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>

	<!-- Danger zone -->
	<div class="rounded-2xl border border-red-500/10 bg-[var(--color-bg-secondary)] p-8">
		<div class="flex items-center justify-between">
			<div>
				<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">Delete Recipe</h2>
				<p class="mt-1 text-sm text-[var(--color-text-secondary)]">This action cannot be undone.</p>
			</div>
			{#if deleteConfirm}
				<div class="flex items-center gap-2">
					<button
						onclick={handleDelete}
						class="rounded-lg bg-red-500 px-4 py-2 text-sm font-medium text-white transition-colors hover:opacity-90"
					>
						Confirm delete
					</button>
					<button
						onclick={() => (deleteConfirm = false)}
						class="rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2 text-sm text-[var(--color-text-secondary)]"
					>
						Cancel
					</button>
				</div>
			{:else}
				<button
					onclick={() => (deleteConfirm = true)}
					class="rounded-lg px-4 py-2 text-sm font-medium text-red-400 transition-colors hover:bg-red-400/10 hover:text-red-300"
				>
					Delete
				</button>
			{/if}
		</div>
	</div>
</div>
