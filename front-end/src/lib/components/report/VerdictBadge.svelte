<script lang="ts">
	import { classificationClasses } from './styles';
	import type { Classification, Confidence } from '$lib/api/types';

	interface Props {
		classification?: Classification | string;
		score?: number;
		confidence?: Confidence;
		size?: 'sm' | 'md';
	}

	let { classification, score, confidence, size = 'md' }: Props = $props();

	const cls = $derived(classificationClasses(classification));
	const label = $derived(
		classification
			? classification.charAt(0).toUpperCase() + classification.slice(1)
			: 'Unknown'
	);
	const padding = $derived(size === 'sm' ? 'px-2 py-0.5 text-xs' : 'px-3 py-1 text-sm');
</script>

<span class="inline-flex items-center gap-2 rounded-full font-medium leading-none {padding} {cls.pill}">
	<span class="h-2 w-2 rounded-full {cls.dot}"></span>
	<span>{label}</span>
	{#if score !== undefined}
		<span class="opacity-80">{score}/100</span>
	{/if}
	{#if confidence}
		<span class="opacity-70">· {confidence}</span>
	{/if}
</span>
