<script lang="ts">
	import type { ArtifactLink, Block } from '$lib/api/types';

	import Markdown from './blocks/Markdown.svelte';
	import Callout from './blocks/Callout.svelte';
	import Heading from './blocks/Heading.svelte';
	import Divider from './blocks/Divider.svelte';
	import Kv from './blocks/Kv.svelte';
	import Table from './blocks/Table.svelte';
	import Code from './blocks/Code.svelte';
	import Json from './blocks/Json.svelte';
	import Hex from './blocks/Hex.svelte';
	import Image from './blocks/Image.svelte';
	import Download from './blocks/Download.svelte';
	import Iocs from './blocks/Iocs.svelte';
	import Ttps from './blocks/Ttps.svelte';
	import Tree from './blocks/Tree.svelte';
	import Timeline from './blocks/Timeline.svelte';
	import Graph from './blocks/Graph.svelte';
	import Unknown from './blocks/Unknown.svelte';

	interface Props {
		block: Block;
		artifacts: ArtifactLink[];
	}
	let { block, artifacts }: Props = $props();
</script>

{#if block.type === 'markdown'}
	<Markdown text={block.text} />
{:else if block.type === 'callout'}
	<Callout level={block.level} text={block.text} />
{:else if block.type === 'heading'}
	<Heading level={block.level} text={block.text} />
{:else if block.type === 'divider'}
	<Divider />
{:else if block.type === 'kv'}
	<Kv pairs={block.pairs} />
{:else if block.type === 'table'}
	<Table
		columns={block.columns}
		rows={block.rows}
		sortable={block.sortable}
		searchable={block.searchable}
	/>
{:else if block.type === 'code'}
	<Code language={block.language} text={block.text} />
{:else if block.type === 'json'}
	<Json data={block.data} collapsed={block.collapsed} />
{:else if block.type === 'hex'}
	<Hex bytes_b64={block.bytes_b64} offset={block.offset} />
{:else if block.type === 'image'}
	<Image artifact={block.artifact} caption={block.caption} {artifacts} />
{:else if block.type === 'download'}
	<Download artifact={block.artifact} label={block.label} {artifacts} />
{:else if block.type === 'iocs'}
	<Iocs items={block.items} />
{:else if block.type === 'ttps'}
	<Ttps items={block.items} />
{:else if block.type === 'tree'}
	<Tree nodes={block.nodes} />
{:else if block.type === 'timeline'}
	<Timeline events={block.events} />
{:else if block.type === 'graph'}
	<Graph nodes={block.nodes} edges={block.edges} />
{:else}
	<Unknown {block} />
{/if}
