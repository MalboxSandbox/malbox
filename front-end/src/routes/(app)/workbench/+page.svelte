<script lang="ts">
	import CodeMirrorEditor from '$lib/components/CodeMirrorEditor.svelte';
	import { transformStore } from '$lib/stores/transforms.svelte';
	import TransformStepEditor from '$lib/components/transforms/TransformStepEditor.svelte';
	import type {
		PipelineStep,
		TransformDefinition,
		PipelineResult,
		TransformCategory
	} from '$lib/transforms';
	import type { PageData } from './$types';
	import type { Recipe, RecipeStep } from '$lib/api/types';
	import { createRecipe } from '$lib/api/recipes';
	import { page } from '$app/stores';
	import { invalidate } from '$app/navigation';
	import { detectFileExtension } from '$lib/transforms/file-type';

	interface Props {
		data: PageData;
	}

	let { data }: Props = $props();

	let inputBytes = $state<Uint8Array>(new Uint8Array());
	let inputText = $state('');
	let steps = $state<PipelineStep[]>([]);
	let activeIndex = $state(-1);
	let result = $state<PipelineResult | null>(null);
	let error = $state<string | null>(null);
	let running = $state(false);
	let searchQuery = $state('');

	let leftWidth = $state(35);
	let middleWidth = $state(25);
	let resizing = $state<'left' | 'right' | null>(null);
	let containerRef = $state<HTMLDivElement | null>(null);

	function handleResizeStart(handle: 'left' | 'right') {
		resizing = handle;
		document.addEventListener('mousemove', handleResizeMove);
		document.addEventListener('mouseup', handleResizeEnd);
	}

	function handleResizeMove(e: MouseEvent) {
		if (!resizing || !containerRef) return;
		const rect = containerRef.getBoundingClientRect();
		const pct = ((e.clientX - rect.left) / rect.width) * 100;
		if (resizing === 'left') {
			leftWidth = Math.max(15, Math.min(pct, 100 - middleWidth - 15));
		} else {
			const newMiddle = pct - leftWidth;
			middleWidth = Math.max(15, Math.min(newMiddle, 100 - leftWidth - 15));
		}
	}

	function handleResizeEnd() {
		resizing = null;
		document.removeEventListener('mousemove', handleResizeMove);
		document.removeEventListener('mouseup', handleResizeEnd);
	}

	let showRecipeSave = $state(false);
	let recipeName = $state('');
	let recipeDescription = $state('');
	let recipeTags = $state('');
	let recipeScope = $state<import('$lib/api/types').RecipeScope>('personal');
	let savingRecipe = $state(false);

	let showRecipeLoad = $state(false);

	$effect(() => {
		const url = new URL($page.url);
		const dataParam = url.searchParams.get('data');
		const stepsParam = url.searchParams.get('steps');
		if (dataParam) {
			try {
				const binary = atob(dataParam);
				const bytes = new Uint8Array(binary.length);
				for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
				inputBytes = bytes;
				try {
					inputText = new TextDecoder().decode(inputBytes);
				} catch {
					inputText = `[${inputBytes.length} bytes - binary]`;
				}
			} catch {
				/* ignore */
			}
		}
		if (stepsParam) {
			try {
				steps = JSON.parse(decodeURIComponent(stepsParam));
				runPipeline();
			} catch {
				/* ignore */
			}
		}
	});

	$effect(() => {
		if (!showRecipeSave && !showRecipeLoad) return;
		function onKey(e: KeyboardEvent) {
			if (e.key === 'Escape') {
				showRecipeSave = false;
				showRecipeLoad = false;
			}
		}
		window.addEventListener('keydown', onKey);
		return () => window.removeEventListener('keydown', onKey);
	});

	function handleInputChange(text: string) {
		inputText = text;
		inputBytes = new TextEncoder().encode(inputText);
		if (steps.length > 0) runPipeline();
	}

	async function runPipeline() {
		if (inputBytes.length === 0) {
			result = null;
			return;
		}
		error = null;
		running = true;
		try {
			result = await transformStore.pipeline.run(inputBytes, steps);
			activeIndex = steps.length - 1;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
			result = null;
		} finally {
			running = false;
		}
	}

	async function runAutoDetectStep() {
		const currentOutput = result?.output ?? inputBytes;
		if (currentOutput.length === 0) return;

		const next = transformStore.autoDetect.peek(currentOutput);
		if (!next) return;

		error = null;
		running = true;
		const newSteps: PipelineStep[] = [
			...steps,
			{ transformId: next.id, params: {}, source: 'auto-detect' }
		];
		try {
			const newResult = await transformStore.pipeline.run(inputBytes, newSteps);
			steps = newSteps;
			result = newResult;
			activeIndex = steps.length - 1;
		} catch (e) {
			error = `${next.name} failed: ${e instanceof Error ? e.message : String(e)}`;
		} finally {
			running = false;
		}
	}

	function addTransform(t: TransformDefinition) {
		const defaultParams: Record<string, string | number | boolean> = {};
		if (t.paramSchema) {
			for (const p of t.paramSchema) {
				if (p.default !== undefined) defaultParams[p.key] = p.default;
			}
		}
		steps = [...steps, { transformId: t.id, params: defaultParams, source: 'manual' }];
		runPipeline();
	}

	function updateStepParams(index: number, params: Record<string, string | number | boolean>) {
		steps = steps.map((s, i) => (i === index ? { ...s, params } : s));
		runPipeline();
	}

	function removeStep(index: number) {
		steps = steps.filter((_, i) => i !== index);
		if (steps.length === 0) {
			result = null;
			activeIndex = -1;
		} else {
			runPipeline();
		}
	}

	let dragSourceIndex = $state<number | null>(null);
	let dragOverIndex = $state<number | null>(null);

	function handleDragStart(index: number) {
		dragSourceIndex = index;
	}

	function handleDragOver(index: number) {
		dragOverIndex = index;
	}

	function handleDragEnd() {
		if (dragSourceIndex !== null && dragOverIndex !== null && dragSourceIndex !== dragOverIndex) {
			const newSteps = [...steps];
			const [moved] = newSteps.splice(dragSourceIndex, 1);
			newSteps.splice(dragOverIndex, 0, moved);
			steps = newSteps;
			runPipeline();
		}
		dragSourceIndex = null;
		dragOverIndex = null;
	}

	function clearAll() {
		steps = [];
		result = null;
		activeIndex = -1;
		error = null;
	}

	async function handleFileUpload(e: Event) {
		const input = e.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;
		const buffer = await file.arrayBuffer();
		inputBytes = new Uint8Array(buffer);
		try {
			inputText = new TextDecoder().decode(inputBytes);
		} catch {
			inputText = `[${inputBytes.length} bytes - binary]`;
		}
		if (steps.length > 0) runPipeline();
	}

	const filteredTransforms = $derived.by(() => {
		const all = transformStore.transforms;
		if (!searchQuery.trim()) return all;
		const q = searchQuery.toLowerCase();
		return all.filter(
			(t) => t.name.toLowerCase().includes(q) || t.id.includes(q) || t.category.includes(q)
		);
	});

	const groupedTransforms = $derived.by(() => {
		const groups: Record<string, TransformDefinition[]> = {};
		for (const t of filteredTransforms) {
			(groups[t.category] ??= []).push(t);
		}
		return groups;
	});

	const categoryOrder: TransformCategory[] = [
		'encoding',
		'compression',
		'crypto',
		'text',
		'analysis',
		'custom'
	];

	const sortedCategories = $derived.by(() => {
		const keys = Object.keys(groupedTransforms);
		return categoryOrder.filter((c) => keys.includes(c));
	});

	const nextDetection = $derived.by(() => {
		const currentData = result?.output ?? inputBytes;
		if (currentData.length === 0) return null;
		return transformStore.autoDetect.peek(currentData);
	});

	const activeOutput = $derived.by(() => {
		if (!result) return null;
		if (activeIndex >= 0) return result.intermediates?.[activeIndex + 1] ?? result.output;
		return inputBytes;
	});

	const outputText = $derived.by(() => {
		if (!activeOutput || activeOutput.length === 0) return '';
		try {
			return new TextDecoder('utf-8', { fatal: true }).decode(activeOutput);
		} catch {
			return new TextDecoder('latin1').decode(activeOutput);
		}
	});

	async function copyOutput() {
		if (!activeOutput) return;
		await navigator.clipboard.writeText(outputText);
	}

	function downloadOutput() {
		if (!activeOutput) return;
		const lastTransformId = steps.length > 0 ? steps[steps.length - 1].transformId : undefined;
		const ext = detectFileExtension(activeOutput, lastTransformId);
		const blob = new Blob([activeOutput.buffer as ArrayBuffer]);
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a');
		a.href = url;
		a.download = `output.${ext}`;
		a.click();
		URL.revokeObjectURL(url);
	}

	async function handleSaveRecipe() {
		if (!recipeName.trim() || steps.length === 0) return;
		savingRecipe = true;
		try {
			const recipeSteps: RecipeStep[] = steps.map((s) => ({
				transform_id: s.transformId,
				params: s.params
			}));
			await createRecipe(fetch, {
				name: recipeName.trim(),
				description: recipeDescription.trim() || undefined,
				author: 'user',
				scope: recipeScope,
				tags: recipeTags
					.split(',')
					.map((t) => t.trim())
					.filter(Boolean),
				steps: recipeSteps
			});
			showRecipeSave = false;
			recipeName = '';
			recipeDescription = '';
			recipeTags = '';
			recipeScope = 'personal';
			await invalidate('malbox:recipes');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			savingRecipe = false;
		}
	}

	function loadRecipe(recipe: Recipe) {
		steps = recipe.steps.map((s) => ({
			transformId: s.transform_id,
			params: s.params,
			source: 'manual' as const
		}));
		showRecipeLoad = false;
		if (inputBytes.length > 0) runPipeline();
	}

	function formatBytes(n: number): string {
		if (n < 1024) return `${n} B`;
		if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
		return `${(n / (1024 * 1024)).toFixed(1)} MB`;
	}
</script>

<div class="flex h-[calc(100vh-4rem)] flex-col gap-3 p-4">
	<!-- Toolbar -->
	<div class="flex items-center justify-between">
		<h1 class="text-xl font-semibold text-[var(--color-text-primary)]">Workbench</h1>
		<div class="flex items-center gap-1.5">
			{#if data.recipes.length > 0}
				<button
					onclick={() => (showRecipeLoad = !showRecipeLoad)}
					class="rounded-lg px-3 py-1.5 text-sm text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-primary)]"
				>
					Load recipe
				</button>
			{/if}
			{#if steps.length > 0}
				<button
					onclick={() => (showRecipeSave = !showRecipeSave)}
					class="rounded-lg px-3 py-1.5 text-sm text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-primary)]"
				>
					Save recipe
				</button>
				<div class="mx-0.5 h-4 w-px bg-[var(--color-border)]"></div>
			{/if}
			<button
				onclick={clearAll}
				class="rounded-lg px-3 py-1.5 text-sm text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-primary)]"
			>
				Clear
			</button>
		</div>
	</div>

	{#if error}
		<div class="rounded-lg bg-[var(--color-error)]/10 px-3 py-2 text-sm text-[var(--color-error)]">
			{error}
		</div>
	{/if}

	<!-- Recipe save overlay -->
	{#if showRecipeSave}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<div
			class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-[2px]"
			onclick={(e) => {
				if (e.target === e.currentTarget) showRecipeSave = false;
			}}
		>
			<div
				class="w-full max-w-md space-y-4 rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-5 shadow-2xl"
			>
				<h2 class="text-base font-semibold text-[var(--color-text-primary)]">Save as Recipe</h2>
				<div class="space-y-3">
					<input
						type="text"
						bind:value={recipeName}
						placeholder="Recipe name"
						class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-1 focus:ring-[var(--color-accent)]/50"
					/>
					<input
						type="text"
						bind:value={recipeDescription}
						placeholder="Description (optional)"
						class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-1 focus:ring-[var(--color-accent)]/50"
					/>
					<div class="grid grid-cols-2 gap-3">
						<input
							type="text"
							bind:value={recipeTags}
							placeholder="Tags (comma-separated)"
							class="rounded-lg bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-1 focus:ring-[var(--color-accent)]/50"
						/>
						<select
							bind:value={recipeScope}
							class="rounded-lg bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-1 focus:ring-[var(--color-accent)]/50"
						>
							<option value="personal">Personal</option>
							<option value="shared">Shared</option>
						</select>
					</div>
				</div>
				<div class="truncate font-mono text-xs text-[var(--color-text-secondary)]">
					{steps.length} step{steps.length !== 1 ? 's' : ''}:
					{steps.map((s) => s.transformId).join(' → ')}
				</div>
				<div class="flex items-center justify-end gap-2">
					<button
						onclick={() => (showRecipeSave = false)}
						class="rounded-lg px-4 py-2 text-sm text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text-primary)]"
					>
						Cancel
					</button>
					<button
						onclick={handleSaveRecipe}
						disabled={savingRecipe || !recipeName.trim()}
						class="rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-[var(--color-accent-hover)] disabled:opacity-40"
					>
						{savingRecipe ? 'Saving...' : 'Save'}
					</button>
				</div>
			</div>
		</div>
	{/if}

	<!-- Recipe load overlay -->
	{#if showRecipeLoad}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<div
			class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-[2px]"
			onclick={(e) => {
				if (e.target === e.currentTarget) showRecipeLoad = false;
			}}
		>
			<div
				class="w-full max-w-md overflow-hidden rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-secondary)] shadow-2xl"
			>
				<div
					class="flex items-center justify-between border-b border-[var(--color-border)]/50 px-5 py-3"
				>
					<h2 class="text-base font-semibold text-[var(--color-text-primary)]">Load Recipe</h2>
					<button
						aria-label="Close"
						onclick={() => (showRecipeLoad = false)}
						class="rounded-md p-1 text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-primary)]"
					>
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 20 20"
							fill="currentColor"
							class="h-4 w-4"
						>
							<path
								d="M6.28 5.22a.75.75 0 00-1.06 1.06L8.94 10l-3.72 3.72a.75.75 0 101.06 1.06L10 11.06l3.72 3.72a.75.75 0 101.06-1.06L11.06 10l3.72-3.72a.75.75 0 00-1.06-1.06L10 8.94 6.28 5.22z"
							/>
						</svg>
					</button>
				</div>
				<div class="max-h-72 overflow-y-auto">
					{#each data.recipes as recipe (recipe.id)}
						<button
							onclick={() => loadRecipe(recipe)}
							class="flex w-full items-start gap-3 border-b border-[var(--color-border)]/30 px-5 py-3 text-left transition-colors last:border-b-0 hover:bg-[var(--color-bg-tertiary)]"
						>
							<div class="min-w-0 flex-1">
								<div class="text-sm font-medium text-[var(--color-text-primary)]">
									{recipe.name}
								</div>
								{#if recipe.description}
									<div class="mt-0.5 text-xs text-[var(--color-text-secondary)]">
										{recipe.description}
									</div>
								{/if}
								<div class="mt-1 truncate font-mono text-xs text-[var(--color-text-secondary)]/70">
									{recipe.steps.map((s) => s.transform_id).join(' → ')}
								</div>
							</div>
							{#if recipe.tags.length > 0}
								<div class="flex shrink-0 flex-wrap gap-1">
									{#each recipe.tags as tag (tag)}
										<span
											class="rounded-md bg-[var(--color-bg-tertiary)] px-1.5 py-0.5 text-xs text-[var(--color-text-secondary)]"
										>
											{tag}
										</span>
									{/each}
								</div>
							{/if}
						</button>
					{:else}
						<div class="px-5 py-8 text-center text-sm text-[var(--color-text-secondary)]">
							No saved recipes
						</div>
					{/each}
				</div>
			</div>
		</div>
	{/if}

	<!-- Main layout -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="flex min-h-0 flex-1"
		bind:this={containerRef}
		class:select-none={resizing !== null}
		class:cursor-col-resize={resizing !== null}
	>
		<!-- Input pane -->
		<div
			class="flex min-h-0 flex-col rounded-lg border border-[var(--color-border)]/50 bg-[var(--color-bg-secondary)]"
			style="width: {leftWidth}%; flex-shrink: 0"
		>
			<div class="flex items-center justify-between px-4 py-2">
				<span
					class="text-xs font-medium uppercase tracking-wider text-[var(--color-text-secondary)]"
					>Input</span
				>
				<div class="flex items-center gap-2">
					<span class="text-xs tabular-nums text-[var(--color-text-secondary)]/60">
						{formatBytes(inputBytes.length)}
					</span>
					<label
						class="cursor-pointer rounded-md bg-[var(--color-bg-tertiary)] px-2.5 py-1 text-xs text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text-primary)]"
					>
						Upload
						<input type="file" class="hidden" onchange={handleFileUpload} />
					</label>
				</div>
			</div>
			<div class="min-h-0 flex-1">
				<CodeMirrorEditor
					value={inputText}
					onchange={handleInputChange}
					showLineNumbers={false}
					placeholder="Paste or type data here, or upload a file..."
				/>
			</div>
		</div>

		<!-- Resize handle left -->
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class="group/handle relative flex w-3 shrink-0 cursor-col-resize items-center justify-center"
			onmousedown={() => handleResizeStart('left')}
		>
			<div
				class="h-8 w-0.5 rounded-full bg-[var(--color-border)]/40 transition-all group-hover/handle:h-12 group-hover/handle:bg-[var(--color-accent)]/50"
			></div>
		</div>

		<!-- Operations + Pipeline pane -->
		<div class="flex min-h-0 flex-col gap-2" style="width: {middleWidth}%; flex-shrink: 0">
			<!-- Operations list -->
			<div
				class="flex min-h-0 flex-1 flex-col rounded-lg border border-[var(--color-border)]/50 bg-[var(--color-bg-secondary)]"
			>
				<div class="px-4 py-2">
					<span
						class="text-xs font-medium uppercase tracking-wider text-[var(--color-text-secondary)]"
						>Transforms</span
					>
				</div>
				<div class="px-2 pb-2">
					<div class="relative">
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 20 20"
							fill="currentColor"
							class="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[var(--color-text-secondary)]/50"
						>
							<path
								fill-rule="evenodd"
								d="M9 3.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11ZM2 9a7 7 0 1 1 12.452 4.391l3.328 3.329a.75.75 0 1 1-1.06 1.06l-3.329-3.328A7 7 0 0 1 2 9Z"
								clip-rule="evenodd"
							/>
						</svg>
						<input
							type="text"
							placeholder="Search..."
							bind:value={searchQuery}
							class="w-full rounded-lg bg-[var(--color-bg-tertiary)] py-1.5 pl-8 pr-3 text-xs text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-1 focus:ring-[var(--color-accent)]/30"
						/>
					</div>
				</div>
				<div class="flex-1 overflow-y-auto px-2 pb-2">
					{#each sortedCategories as category (category)}
						<div class="mb-1.5">
							<div
								class="px-2 py-1 text-xs font-medium uppercase tracking-wider text-[var(--color-text-secondary)]/60"
							>
								{category}
							</div>
							{#each groupedTransforms[category] as t (t.id)}
								<button
									onclick={() => addTransform(t)}
									class="flex w-full items-center gap-1.5 rounded-md px-2.5 py-1.5 text-left text-xs text-[var(--color-text-primary)] transition-colors hover:bg-[var(--color-bg-tertiary)]"
								>
									<span class="truncate">{t.name}</span>
									{#if t.paramSchema && t.paramSchema.length > 0}
										<svg
											xmlns="http://www.w3.org/2000/svg"
											viewBox="0 0 16 16"
											fill="currentColor"
											class="h-3 w-3 shrink-0 text-[var(--color-text-secondary)]/50"
										>
											<path
												fill-rule="evenodd"
												d="M6.955 1.45A.5.5 0 0 1 7.452 1h1.096a.5.5 0 0 1 .497.45l.17 1.699c.484.12.94.312 1.356.562l1.321-.916a.5.5 0 0 1 .67.055l.774.774a.5.5 0 0 1 .055.67l-.916 1.321c.25.417.443.872.562 1.356l1.699.17a.5.5 0 0 1 .45.497v1.096a.5.5 0 0 1-.45.497l-1.699.17c-.12.484-.312.94-.562 1.356l.916 1.321a.5.5 0 0 1-.055.67l-.774.774a.5.5 0 0 1-.67.055l-1.321-.916c-.417.25-.872.443-1.356.562l-.17 1.699a.5.5 0 0 1-.497.45H7.452a.5.5 0 0 1-.497-.45l-.17-1.699a4.973 4.973 0 0 1-1.356-.562l-1.321.916a.5.5 0 0 1-.67-.055l-.774-.774a.5.5 0 0 1-.055-.67l.916-1.321a4.973 4.973 0 0 1-.562-1.356l-1.699-.17A.5.5 0 0 1 1 8.548V7.452a.5.5 0 0 1 .45-.497l1.699-.17c.12-.484.312-.94.562-1.356L2.795 4.108a.5.5 0 0 1 .055-.67l.774-.774a.5.5 0 0 1 .67-.055l1.321.916c.417-.25.872-.443 1.356-.562l.17-1.699ZM8 10.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5Z"
												clip-rule="evenodd"
											/>
										</svg>
									{/if}
									{#if t.terminal}
										<span
											class="shrink-0 rounded bg-amber-500/10 px-1 py-0.5 text-xs leading-none text-amber-400"
										>
											end
										</span>
									{/if}
									{#if t.provenance !== 'builtin'}
										<span class="shrink-0 text-xs text-[var(--color-text-secondary)]/60">
											({t.provenance})
										</span>
									{/if}
								</button>
							{/each}
						</div>
					{/each}
					{#if sortedCategories.length === 0}
						<p class="px-3 py-4 text-center text-xs text-[var(--color-text-secondary)]">
							No transforms found
						</p>
					{/if}
				</div>
			</div>

			<!-- Pipeline steps -->
			{#if steps.length > 0}
				<div
					class="flex max-h-[40%] flex-col rounded-lg border border-[var(--color-border)]/50 bg-[var(--color-bg-secondary)]"
				>
					<div class="flex items-center justify-between px-4 py-2">
						<div class="flex items-center gap-2">
							<span
								class="text-xs font-medium uppercase tracking-wider text-[var(--color-text-secondary)]"
								>Pipeline</span
							>
							<span
								class="rounded-md bg-[var(--color-accent)]/10 px-1.5 py-0.5 text-xs font-medium tabular-nums text-[var(--color-accent)]"
							>
								{steps.length}
							</span>
						</div>
					</div>
					<div class="flex-1 space-y-1 overflow-y-auto p-2">
						{#each steps as step, i (i)}
							<TransformStepEditor
								{step}
								index={i}
								onUpdate={(params) => updateStepParams(i, params)}
								onRemove={() => removeStep(i)}
								onDragStart={handleDragStart}
								onDragOver={handleDragOver}
								onDragEnd={handleDragEnd}
								dragOver={dragOverIndex === i}
							/>
						{/each}
					</div>
				</div>
			{/if}
		</div>

		<!-- Resize handle right -->
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class="group/handle relative flex w-3 shrink-0 cursor-col-resize items-center justify-center"
			onmousedown={() => handleResizeStart('right')}
		>
			<div
				class="h-8 w-0.5 rounded-full bg-[var(--color-border)]/40 transition-all group-hover/handle:h-12 group-hover/handle:bg-[var(--color-accent)]/50"
			></div>
		</div>

		<!-- Output pane -->
		<div
			class="flex min-h-0 flex-1 flex-col rounded-lg border border-[var(--color-border)]/50 bg-[var(--color-bg-secondary)]"
		>
			<div class="flex items-center justify-between px-4 py-2">
				<div class="flex items-center gap-2">
					<span
						class="text-xs font-medium uppercase tracking-wider text-[var(--color-text-secondary)]"
						>Output</span
					>
					{#if nextDetection}
						<button
							onclick={runAutoDetectStep}
							disabled={running}
							class="flex items-center gap-1.5 rounded-md px-1.5 py-1 text-[var(--color-accent)] transition-colors hover:bg-[var(--color-accent)]/10 disabled:opacity-40"
							title="Auto-detect: {nextDetection.name}"
						>
							{#if running}
								<div
									class="h-4 w-4 animate-spin rounded-full border-2 border-[var(--color-accent)] border-t-transparent"
								></div>
							{:else}
								<svg
									xmlns="http://www.w3.org/2000/svg"
									viewBox="0 0 24 24"
									fill="currentColor"
									class="h-4 w-4 shrink-0"
								>
									<path
										fill-rule="evenodd"
										d="M9 4.5a.75.75 0 0 1 .721.544l.813 2.846a3.75 3.75 0 0 0 2.576 2.576l2.846.813a.75.75 0 0 1 0 1.442l-2.846.813a3.75 3.75 0 0 0-2.576 2.576l-.813 2.846a.75.75 0 0 1-1.442 0l-.813-2.846a3.75 3.75 0 0 0-2.576-2.576l-2.846-.813a.75.75 0 0 1 0-1.442l2.846-.813A3.75 3.75 0 0 0 7.466 7.89l.813-2.846A.75.75 0 0 1 9 4.5ZM18 1.5a.75.75 0 0 1 .728.568l.258 1.036c.236.94.97 1.674 1.91 1.91l1.036.258a.75.75 0 0 1 0 1.456l-1.036.258c-.94.236-1.674.97-1.91 1.91l-.258 1.036a.75.75 0 0 1-1.456 0l-.258-1.036a2.625 2.625 0 0 0-1.91-1.91l-1.036-.258a.75.75 0 0 1 0-1.456l1.036-.258a2.625 2.625 0 0 0 1.91-1.91l.258-1.036A.75.75 0 0 1 18 1.5ZM16.5 15a.75.75 0 0 1 .712.513l.394 1.183c.15.447.5.799.948.948l1.183.395a.75.75 0 0 1 0 1.422l-1.183.395c-.447.15-.799.5-.948.948l-.395 1.183a.75.75 0 0 1-1.422 0l-.395-1.183a1.5 1.5 0 0 0-.948-.948l-1.183-.395a.75.75 0 0 1 0-1.422l1.183-.395c.447-.15.799-.5.948-.948l.395-1.183A.75.75 0 0 1 16.5 15Z"
										clip-rule="evenodd"
									/>
								</svg>
								<span class="text-xs text-[var(--color-accent)]/70">{nextDetection?.name}</span>
							{/if}
						</button>
					{/if}
				</div>
				<div class="flex items-center gap-1">
					{#if activeOutput}
						<span class="mr-1 text-xs tabular-nums text-[var(--color-text-secondary)]">
							{formatBytes(activeOutput.length)}
						</span>
						<button
							onclick={copyOutput}
							class="rounded-md p-1.5 text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-primary)]"
							title="Copy to clipboard"
						>
							<svg
								xmlns="http://www.w3.org/2000/svg"
								viewBox="0 0 20 20"
								fill="currentColor"
								class="h-4 w-4"
							>
								<path
									d="M7 3.5A1.5 1.5 0 0 1 8.5 2h3.879a1.5 1.5 0 0 1 1.06.44l3.122 3.12A1.5 1.5 0 0 1 17 6.622V12.5a1.5 1.5 0 0 1-1.5 1.5h-1v-3.379a3 3 0 0 0-.879-2.121L10.5 5.379A3 3 0 0 0 8.379 4.5H7v-1Z"
								/>
								<path
									d="M4.5 6A1.5 1.5 0 0 0 3 7.5v9A1.5 1.5 0 0 0 4.5 18h7a1.5 1.5 0 0 0 1.5-1.5v-5.879a1.5 1.5 0 0 0-.44-1.06L9.44 6.439A1.5 1.5 0 0 0 8.378 6H4.5Z"
								/>
							</svg>
						</button>
						<button
							onclick={downloadOutput}
							class="rounded-md p-1.5 text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-primary)]"
							title="Download output"
						>
							<svg
								xmlns="http://www.w3.org/2000/svg"
								viewBox="0 0 20 20"
								fill="currentColor"
								class="h-4 w-4"
							>
								<path
									d="M10.75 2.75a.75.75 0 0 0-1.5 0v8.614L6.295 8.235a.75.75 0 1 0-1.09 1.03l4.25 4.5a.75.75 0 0 0 1.09 0l4.25-4.5a.75.75 0 0 0-1.09-1.03l-2.955 3.129V2.75Z"
								/>
								<path
									d="M3.5 12.75a.75.75 0 0 0-1.5 0v2.5A2.75 2.75 0 0 0 4.75 18h10.5A2.75 2.75 0 0 0 18 15.25v-2.5a.75.75 0 0 0-1.5 0v2.5c0 .69-.56 1.25-1.25 1.25H4.75c-.69 0-1.25-.56-1.25-1.25v-2.5Z"
								/>
							</svg>
						</button>
					{/if}
				</div>
			</div>

			<!-- Output content -->
			{#if running}
				<div class="flex flex-1 items-center justify-center">
					<div
						class="h-6 w-6 animate-spin rounded-full border-2 border-[var(--color-accent)] border-t-transparent"
					></div>
				</div>
			{:else if activeOutput}
				<div class="min-h-0 flex-1">
					<CodeMirrorEditor value={outputText} readonly showLineNumbers={false} lineWrapping />
				</div>
			{:else}
				<div class="flex flex-1 flex-col items-center justify-center gap-3">
					{#if nextDetection}
						<button
							onclick={runAutoDetectStep}
							class="group flex flex-col items-center gap-2 rounded-xl px-6 py-4 transition-colors hover:bg-[var(--color-bg-tertiary)]/50"
						>
							<svg
								xmlns="http://www.w3.org/2000/svg"
								viewBox="0 0 24 24"
								fill="currentColor"
								class="h-8 w-8 text-[var(--color-accent)]/30 transition-colors group-hover:text-[var(--color-accent)]"
							>
								<path
									fill-rule="evenodd"
									d="M9 4.5a.75.75 0 0 1 .721.544l.813 2.846a3.75 3.75 0 0 0 2.576 2.576l2.846.813a.75.75 0 0 1 0 1.442l-2.846.813a3.75 3.75 0 0 0-2.576 2.576l-.813 2.846a.75.75 0 0 1-1.442 0l-.813-2.846a3.75 3.75 0 0 0-2.576-2.576l-2.846-.813a.75.75 0 0 1 0-1.442l2.846-.813A3.75 3.75 0 0 0 7.466 7.89l.813-2.846A.75.75 0 0 1 9 4.5ZM18 1.5a.75.75 0 0 1 .728.568l.258 1.036c.236.94.97 1.674 1.91 1.91l1.036.258a.75.75 0 0 1 0 1.456l-1.036.258c-.94.236-1.674.97-1.91 1.91l-.258 1.036a.75.75 0 0 1-1.456 0l-.258-1.036a2.625 2.625 0 0 0-1.91-1.91l-1.036-.258a.75.75 0 0 1 0-1.456l1.036-.258a2.625 2.625 0 0 0 1.91-1.91l.258-1.036A.75.75 0 0 1 18 1.5ZM16.5 15a.75.75 0 0 1 .712.513l.394 1.183c.15.447.5.799.948.948l1.183.395a.75.75 0 0 1 0 1.422l-1.183.395c-.447.15-.799.5-.948.948l-.395 1.183a.75.75 0 0 1-1.422 0l-.395-1.183a1.5 1.5 0 0 0-.948-.948l-1.183-.395a.75.75 0 0 1 0-1.422l1.183-.395c.447-.15.799-.5.948-.948l.395-1.183A.75.75 0 0 1 16.5 15Z"
									clip-rule="evenodd"
								/>
							</svg>
							<span
								class="text-xs text-[var(--color-text-secondary)]/40 transition-colors group-hover:text-[var(--color-text-secondary)]"
								>{nextDetection?.name}</span
							>
						</button>
					{:else if inputBytes.length > 0}
						<span class="text-sm text-[var(--color-text-secondary)]/40">
							No transforms detected
						</span>
					{:else}
						<span class="text-sm text-[var(--color-text-secondary)]/40">
							Output will appear here
						</span>
					{/if}
				</div>
			{/if}
		</div>
	</div>
</div>
