<script lang="ts">
	import Icon from '$lib/components/ui/Icon.svelte';
	import { icons, type IconName } from '$lib/icons';
	import { GlobeIcon } from '@lucide/svelte';
	import type { MenuItemAction, MenuItemSubmenu } from './types';

	interface Props {
		item: MenuItemAction | MenuItemSubmenu;
		focused: boolean;
		onactivate?: () => void;
		onhover?: () => void;
		onleave?: (e: PointerEvent) => void;
	}
	let { item, focused, onactivate, onhover, onleave }: Props = $props();

	const imageUrl = $derived(item.kind === 'action' ? item.imageUrl : undefined);
	const hasIcon = $derived(item.icon && item.icon in icons);
	const isSubmenu = $derived(item.kind === 'submenu');
	const shortcut = $derived(item.kind === 'action' ? item.shortcut : undefined);
	const disabled = $derived(item.kind === 'action' ? item.disabled : false);

	let imgFailed = $state(false);
</script>

<button
	type="button"
	class="flex w-full items-center gap-2.5 rounded-md px-3 py-1.5 text-sm transition-colors
		{focused
		? 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-primary)]'
		: 'text-[var(--color-text-primary)]'}
		{disabled ? 'opacity-40 cursor-not-allowed' : 'hover:bg-[var(--color-bg-tertiary)] cursor-default'}"
	role="menuitem"
	tabindex={-1}
	aria-disabled={disabled}
	aria-haspopup={isSubmenu ? 'menu' : undefined}
	onclick={onactivate}
	onpointerenter={onhover}
	onpointerleave={onleave}
>
	{#if imageUrl && !imgFailed}
		<img
			src={imageUrl}
			alt=""
			class="size-4 shrink-0 rounded-sm"
			onerror={() => (imgFailed = true)}
		/>
	{:else if imageUrl && imgFailed}
		<GlobeIcon class="size-4 shrink-0 text-[var(--color-text-secondary)]" />
	{:else if hasIcon}
		<Icon
			path={icons[item.icon as IconName]}
			class="size-4 shrink-0 text-[var(--color-text-secondary)]"
		/>
	{/if}

	<span class="flex-1 text-left">{item.label}</span>

	{#if shortcut}
		<span
			class="ml-4 rounded bg-[var(--color-bg-tertiary)] px-1 font-mono text-xs text-[var(--color-text-secondary)]"
		>
			{shortcut}
		</span>
	{/if}

	{#if isSubmenu}
		<Icon path={icons.chevronRight} class="size-3.5 shrink-0 text-[var(--color-text-secondary)]" />
	{/if}
</button>
