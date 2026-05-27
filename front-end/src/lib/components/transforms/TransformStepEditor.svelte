<script lang="ts">
	import { transformStore } from '$lib/stores/transforms.svelte';
	import type { PipelineStep, TransformParamSchema } from '$lib/transforms';

	interface Props {
		step: PipelineStep;
		index: number;
		onUpdate: (params: Record<string, string | number | boolean>) => void;
		onRemove: () => void;
		onDragStart: (index: number) => void;
		onDragOver: (index: number) => void;
		onDragEnd: () => void;
		dragOver: boolean;
	}

	let { step, index, onUpdate, onRemove, onDragStart, onDragOver, onDragEnd, dragOver }: Props =
		$props();

	const transform = $derived(transformStore.registry.get(step.transformId));
	const schema = $derived(transform?.paramSchema ?? []);
	let localParams = $state<Record<string, string | number | boolean>>({ ...step.params });
	let expanded = $state(false);

	$effect(() => {
		localParams = { ...step.params };
		expanded = schema.length > 0 && Object.keys(step.params).length === 0;
	});

	function handleParamChange(key: string, value: string | number | boolean) {
		localParams[key] = value;
		onUpdate({ ...localParams });
	}

	function inputValue(param: TransformParamSchema): string | number | boolean {
		if (localParams[param.key] !== undefined) return localParams[param.key];
		if (param.default !== undefined) return param.default;
		if (param.type === 'number') return 0;
		if (param.type === 'boolean') return false;
		return '';
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="rounded-lg border bg-[var(--color-bg-secondary)] text-xs transition-colors {dragOver
		? 'border-[var(--color-accent)]'
		: 'border-[var(--color-border)]/40'}"
	ondragover={(e) => {
		e.preventDefault();
		onDragOver(index);
	}}
	ondragend={onDragEnd}
>
	<div class="flex items-center gap-1.5 px-2 py-1.5">
		<span
			class="flex h-5 w-5 shrink-0 items-center justify-center rounded text-[10px] font-medium tabular-nums text-[var(--color-text-secondary)]/60"
		>
			{index + 1}
		</span>
		<button
			draggable="true"
			ondragstart={() => onDragStart(index)}
			class="cursor-grab text-[var(--color-text-secondary)]/50 hover:text-[var(--color-text-primary)] active:cursor-grabbing"
			aria-label="Drag to reorder step {index + 1}"
		>
			<svg
				xmlns="http://www.w3.org/2000/svg"
				viewBox="0 0 16 16"
				fill="currentColor"
				class="h-3 w-3"
			>
				<path
					d="M10 4.5a1.5 1.5 0 1 0-3 0 1.5 1.5 0 0 0 3 0ZM10 8a1.5 1.5 0 1 0-3 0 1.5 1.5 0 0 0 3 0ZM10 11.5a1.5 1.5 0 1 0-3 0 1.5 1.5 0 0 0 3 0Z"
				/>
			</svg>
		</button>
		<span class="font-medium text-[var(--color-text-primary)]">
			{transform?.name ?? step.transformId}
		</span>
		{#if step.source === 'auto-detect'}
			<span
				class="rounded bg-[var(--color-accent)]/10 px-1.5 py-0.5 text-[10px] text-[var(--color-accent)]"
			>
				auto
			</span>
		{/if}
		{#if transform?.terminal}
			<span class="rounded bg-amber-500/10 px-1.5 py-0.5 text-[10px] text-amber-400">
				terminal
			</span>
		{/if}
		<div class="flex-1"></div>
		{#if schema.length > 0}
			<button
				onclick={() => (expanded = !expanded)}
				class="rounded p-1 text-[var(--color-text-secondary)]/60 transition-colors hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-primary)]"
				title={expanded ? 'Collapse params' : 'Edit params'}
			>
				<svg
					xmlns="http://www.w3.org/2000/svg"
					viewBox="0 0 20 20"
					fill="currentColor"
					class="h-3.5 w-3.5 transition-transform {expanded ? 'rotate-180' : ''}"
				>
					<path
						fill-rule="evenodd"
						d="M5.23 7.21a.75.75 0 011.06.02L10 11.168l3.71-3.938a.75.75 0 111.08 1.04l-4.25 4.5a.75.75 0 01-1.08 0l-4.25-4.5a.75.75 0 01.02-1.06z"
						clip-rule="evenodd"
					/>
				</svg>
			</button>
		{/if}
		<button
			onclick={onRemove}
			class="rounded p-1 text-[var(--color-text-secondary)]/60 transition-colors hover:bg-[var(--color-error)]/10 hover:text-[var(--color-error)]"
			title="Remove step"
		>
			<svg
				xmlns="http://www.w3.org/2000/svg"
				viewBox="0 0 20 20"
				fill="currentColor"
				class="h-3.5 w-3.5"
			>
				<path
					d="M6.28 5.22a.75.75 0 00-1.06 1.06L8.94 10l-3.72 3.72a.75.75 0 101.06 1.06L10 11.06l3.72 3.72a.75.75 0 101.06-1.06L11.06 10l3.72-3.72a.75.75 0 00-1.06-1.06L10 8.94 6.28 5.22z"
				/>
			</svg>
		</button>
	</div>

	{#if expanded && schema.length > 0}
		<div class="space-y-2 border-t border-[var(--color-border)]/30 px-3 py-2">
			{#each schema as param (param.key)}
				<div>
					<label
						for="param-{step.transformId}-{param.key}"
						class="mb-1 flex items-center gap-1 text-[var(--color-text-secondary)]"
					>
						{param.label}
						{#if param.required}
							<span class="text-[var(--color-error)]">*</span>
						{/if}
					</label>
					{#if param.type === 'select' && param.options}
						<select
							id="param-{step.transformId}-{param.key}"
							value={String(inputValue(param))}
							onchange={(e) => handleParamChange(param.key, e.currentTarget.value)}
							class="w-full rounded bg-[var(--color-bg-tertiary)] px-2 py-1.5 text-[var(--color-text-primary)] focus:outline-none focus:ring-1 focus:ring-[var(--color-accent)]/50"
						>
							{#each param.options as opt (opt)}
								<option value={opt}>{opt}</option>
							{/each}
						</select>
					{:else if param.type === 'boolean'}
						<label class="flex cursor-pointer items-center gap-2">
							<input
								type="checkbox"
								checked={Boolean(inputValue(param))}
								onchange={(e) => handleParamChange(param.key, e.currentTarget.checked)}
								class="rounded border-[var(--color-border)]"
							/>
							<span class="text-[var(--color-text-primary)]">{param.description ?? ''}</span>
						</label>
					{:else if param.type === 'number'}
						<input
							id="param-{step.transformId}-{param.key}"
							type="number"
							value={Number(inputValue(param))}
							oninput={(e) => handleParamChange(param.key, Number(e.currentTarget.value))}
							placeholder={param.description ?? ''}
							class="w-full rounded bg-[var(--color-bg-tertiary)] px-2 py-1.5 font-mono text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)]/50 focus:outline-none focus:ring-1 focus:ring-[var(--color-accent)]/50"
						/>
					{:else}
						<input
							id="param-{step.transformId}-{param.key}"
							type="text"
							value={String(inputValue(param))}
							oninput={(e) => handleParamChange(param.key, e.currentTarget.value)}
							placeholder={param.description ?? ''}
							class="w-full rounded bg-[var(--color-bg-tertiary)] px-2 py-1.5 font-mono text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)]/50 focus:outline-none focus:ring-1 focus:ring-[var(--color-accent)]/50"
						/>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>
