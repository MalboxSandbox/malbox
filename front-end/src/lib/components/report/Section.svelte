<script lang="ts">
	import BlockRenderer from './BlockRenderer.svelte';
	import type { ArtifactLink, Section } from '$lib/api/types';

	interface Props {
		section: Section;
		artifacts: ArtifactLink[];
	}
	let { section, artifacts }: Props = $props();
</script>

<section
	id="section-{section.id}"
	class="scroll-mt-20 space-y-4 rounded-2xl bg-[var(--color-bg-secondary)] p-8"
>
	<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">{section.title}</h2>
	{#if section.blocks && section.blocks.length > 0}
		<div class="space-y-4">
			{#each section.blocks as b, i (i)}
				<BlockRenderer block={b} {artifacts} />
			{/each}
		</div>
	{/if}
</section>
