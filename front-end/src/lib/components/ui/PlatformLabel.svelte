<script lang="ts">
	import Icon from './Icon.svelte';
	import { icons } from '$lib/icons';

	interface Props {
		platform: string;
		class?: string;
	}
	let { platform, class: className = '' }: Props = $props();

	const icon = $derived(
		platform.toLowerCase() === 'windows'
			? icons.windows
			: platform.toLowerCase() === 'linux'
				? icons.linux
				: null
	);
</script>

<span class="inline-flex items-center gap-1.5 {className}">
	{#if icon}
		<Icon path={icon} class="size-3.5 shrink-0" />
	{/if}
	{platform}
</span>
