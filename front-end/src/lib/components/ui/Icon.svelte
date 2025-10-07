<script lang="ts">
	let {
		path,
		viewBox = '0 0 18 18',
		class: className = 'w-5 h-5',
		fill = 'currentColor',
		stroke = 'none',
		strokeWidth = 0,
		...restProps
	}: {
		path: string | string[];
		viewBox?: string;
		class?: string;
		fill?: string;
		stroke?: string;
		strokeWidth?: number;
		[key: string]: any;
	} = $props();

	const paths = $derived(Array.isArray(path) ? path : [path]);
</script>

<svg
	{viewBox}
	{fill}
	{stroke}
	stroke-width={strokeWidth}
	class={className}
	xmlns="http://www.w3.org/2000/svg"
	{...restProps}
>
	{#each paths as p}
		<path d={p} />
	{/each}
</svg>
