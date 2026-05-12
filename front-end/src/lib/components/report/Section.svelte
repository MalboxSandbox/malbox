<script lang="ts">
	import BlockRenderer from './BlockRenderer.svelte';
	import type { ArtifactLink, Block, Section } from '$lib/api/types';

	interface Props {
		section: Section;
		artifacts: ArtifactLink[];
	}
	let { section, artifacts }: Props = $props();

	function isCompact(block: Block): boolean {
		switch (block.type) {
			case 'kv':
				return block.pairs.length <= 8;
			case 'callout':
			case 'download':
			case 'image':
				return true;
			case 'json':
				return block.collapsed === true;
			default:
				return false;
		}
	}

	type LayoutRow = { kind: 'wide'; block: Block } | { kind: 'pair'; blocks: Block[] };

	const rows = $derived.by<LayoutRow[]>(() => {
		const blocks = section.blocks ?? [];
		const result: LayoutRow[] = [];
		let compactRun: Block[] = [];

		function flushCompact() {
			if (compactRun.length === 0) return;
			for (let i = 0; i < compactRun.length; i += 2) {
				result.push({ kind: 'pair', blocks: compactRun.slice(i, i + 2) });
			}
			compactRun = [];
		}

		for (const b of blocks) {
			if (isCompact(b)) {
				compactRun.push(b);
			} else {
				flushCompact();
				result.push({ kind: 'wide', block: b });
			}
		}
		flushCompact();
		return result;
	});
</script>

<section
	id="section-{section.id}"
	class="scroll-mt-20 space-y-4 rounded-2xl bg-[var(--color-bg-secondary)] p-8"
>
	<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">{section.title}</h2>
	{#if rows.length > 0}
		<div class="space-y-4">
			{#each rows as row, i (i)}
				{#if row.kind === 'wide'}
					<BlockRenderer block={row.block} {artifacts} />
				{:else}
					<div class="grid gap-4 md:grid-cols-2">
						{#each row.blocks as b, j (j)}
							<div>
								<BlockRenderer block={b} {artifacts} />
							</div>
						{/each}
					</div>
				{/if}
			{/each}
		</div>
	{/if}
</section>
