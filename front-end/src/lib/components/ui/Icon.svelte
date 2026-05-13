<script lang="ts">
	import type { IconDef } from '$lib/icons';

	let {
		path,
		viewBox = '0 0 18 18',
		class: className = 'w-5 h-5',
		fill = 'currentColor',
		stroke = 'none',
		strokeWidth = 0,
		...restProps
	}: {
		path: IconDef;
		viewBox?: string;
		class?: string;
		fill?: string;
		stroke?: string;
		strokeWidth?: number;
		// eslint-disable-next-line @typescript-eslint/no-explicit-any -- rest props spread
		[key: string]: any;
	} = $props();

	const isConfig = $derived(
		typeof path === 'object' && !Array.isArray(path) && 'd' in (path as object)
	);
	const config = $derived(
		isConfig
			? (path as { d: string | readonly string[]; viewBox?: string; fillRule?: string })
			: null
	);
	const resolvedViewBox = $derived(config?.viewBox ?? viewBox);
	const fillRule = $derived(config?.fillRule as 'evenodd' | 'nonzero' | undefined);
	const paths = $derived(
		config
			? Array.isArray(config.d)
				? config.d
				: [config.d]
			: Array.isArray(path)
				? (path as readonly string[])
				: [path as string]
	);
</script>

<svg
	viewBox={resolvedViewBox}
	{fill}
	{stroke}
	stroke-width={strokeWidth}
	class={className}
	xmlns="http://www.w3.org/2000/svg"
	{...restProps}
>
	{#each paths as p, i (i)}
		{#if fillRule}
			<path d={p} fill-rule={fillRule} clip-rule={fillRule} />
		{:else}
			<path d={p} />
		{/if}
	{/each}
</svg>
