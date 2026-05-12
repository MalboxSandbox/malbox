<script lang="ts">
	interface Props {
		value: string;
		size?: 'sm' | 'md';
	}
	let { value, size = 'md' }: Props = $props();

	let copied = $state(false);

	async function copy() {
		await navigator.clipboard.writeText(value);
		copied = true;
		setTimeout(() => {
			copied = false;
		}, 1500);
	}

	const iconSize = $derived(size === 'sm' ? 'size-3.5' : 'size-4');
</script>

<button
	type="button"
	class="relative shrink-0 transition-colors
		{copied
		? 'text-[var(--color-accent)]'
		: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
	onclick={copy}
	title="Copy to clipboard"
>
	<svg
		xmlns="http://www.w3.org/2000/svg"
		viewBox="0 0 20 20"
		fill="currentColor"
		class="{iconSize} transition-all duration-200
			{copied ? 'scale-0 opacity-0' : 'scale-100 opacity-100'}"
	>
		<path
			d="M7 3.5A1.5 1.5 0 0 1 8.5 2h3.879a1.5 1.5 0 0 1 1.06.44l3.122 3.12A1.5 1.5 0 0 1 17 6.622V12.5a1.5 1.5 0 0 1-1.5 1.5h-1v-3.379a3 3 0 0 0-.879-2.121L10.5 5.379A3 3 0 0 0 8.379 4.5H7v-1Z"
		/>
		<path
			d="M4.5 6A1.5 1.5 0 0 0 3 7.5v9A1.5 1.5 0 0 0 4.5 18h7a1.5 1.5 0 0 0 1.5-1.5v-5.879a1.5 1.5 0 0 0-.44-1.06L9.44 6.439A1.5 1.5 0 0 0 8.378 6H4.5Z"
		/>
	</svg>
	<svg
		xmlns="http://www.w3.org/2000/svg"
		viewBox="0 0 20 20"
		fill="currentColor"
		class="absolute inset-0 {iconSize} transition-all duration-200
			{copied ? 'scale-100 opacity-100' : 'scale-0 opacity-0'}"
	>
		<path
			fill-rule="evenodd"
			d="M16.704 4.153a.75.75 0 0 1 .143 1.052l-8 10.5a.75.75 0 0 1-1.127.075l-4.5-4.5a.75.75 0 0 1 1.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 0 1 1.05-.143Z"
			clip-rule="evenodd"
		/>
	</svg>
</button>
