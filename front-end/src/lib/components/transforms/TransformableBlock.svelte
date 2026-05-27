<script lang="ts">
	import { transformStore } from '$lib/stores/transforms.svelte';
	import TransformTrail from './TransformTrail.svelte';
	import TransformMenu from './TransformMenu.svelte';
	import type { PipelineStep, TransformDefinition, PipelineResult } from '$lib/transforms';
	import type { Snippet } from 'svelte';

	interface Props {
		data: Uint8Array;
		renderTransformed: Snippet<[Uint8Array]>;
		renderOriginal: Snippet;
	}

	let { data, renderTransformed, renderOriginal }: Props = $props();

	let steps = $state<PipelineStep[]>([]);
	let activeIndex = $state(-1);
	let pipelineResult = $state<PipelineResult | null>(null);
	let showRaw = $state(false);
	let error = $state<string | null>(null);
	let loading = $state(false);

	const displayData = $derived.by(() => {
		if (showRaw || !pipelineResult || steps.length === 0) return null;
		if (activeIndex === -1) return null;
		return pipelineResult.intermediates[activeIndex + 1] ?? pipelineResult.output;
	});

	function handleRewind(index: number) {
		activeIndex = index;
	}

	function handleRemoveStep(index: number) {
		steps = steps.filter((_, i) => i !== index);
		if (steps.length === 0) {
			activeIndex = -1;
			pipelineResult = null;
		} else if (activeIndex >= steps.length) {
			activeIndex = steps.length - 1;
		}
		rerunPipeline();
	}

	async function handleAddTransform(transform: TransformDefinition) {
		const newStep: PipelineStep = {
			transformId: transform.id,
			params: {},
			source: 'manual'
		};
		steps = [...steps, newStep];
		await rerunPipeline();
		activeIndex = steps.length - 1;
	}

	async function rerunPipeline() {
		if (steps.length === 0) {
			pipelineResult = null;
			error = null;
			return;
		}
		loading = true;
		try {
			const result = await transformStore.pipeline.run(data, steps);
			pipelineResult = result;
			activeIndex = steps.length - 1;
			error = null;
		} catch (e) {
			pipelineResult = null;
			error = e instanceof Error ? e.message : String(e);
			console.warn('TransformableBlock pipeline failed:', e);
		} finally {
			loading = false;
		}
	}

	function openInWorkbench() {
		let encoded = '';
		for (let i = 0; i < data.length; i++) encoded += String.fromCharCode(data[i]);
		encoded = btoa(encoded);
		const stepsJson = encodeURIComponent(JSON.stringify(steps));
		window.location.href = `/workbench?data=${encoded}&steps=${stepsJson}`;
	}
</script>

<div class="group relative">
	<div
		class="absolute right-2 top-2 z-10 flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100"
	>
		{#if loading}
			<div
				class="h-4 w-4 animate-spin rounded-full border-2 border-[var(--color-accent)] border-t-transparent"
			></div>
		{/if}
		<TransformMenu onSelect={handleAddTransform} />
		{#if steps.length > 0}
			<button
				onclick={openInWorkbench}
				class="rounded p-1.5 text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-primary)]"
				title="Open in Workbench"
			>
				<svg
					xmlns="http://www.w3.org/2000/svg"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					class="h-4 w-4"
				>
					<path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
					<polyline points="15 3 21 3 21 9" />
					<line x1="10" y1="14" x2="21" y2="3" />
				</svg>
			</button>
			<button
				onclick={() => (showRaw = !showRaw)}
				class="rounded px-2 py-1 text-[10px] font-medium transition-colors {showRaw
					? 'bg-[var(--color-accent)] text-white'
					: 'text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-tertiary)]'}"
			>
				{showRaw ? 'raw' : 'decoded'}
			</button>
		{/if}
	</div>

	{#if error}
		<div
			class="absolute left-2 top-2 z-10 rounded bg-[var(--color-error)]/10 px-2 py-1 text-[10px] text-[var(--color-error)]"
			title={error}
		>
			Transform error
		</div>
	{/if}

	<TransformTrail {steps} {activeIndex} onRewind={handleRewind} onRemoveStep={handleRemoveStep} />

	{#if displayData && !showRaw}
		{@render renderTransformed(displayData)}
	{:else}
		{@render renderOriginal()}
	{/if}
</div>
