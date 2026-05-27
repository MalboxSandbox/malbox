<script lang="ts">
	import type { MenuItem as MenuItemType } from './types';
	import MenuItem from './MenuItem.svelte';
	import MenuSeparator from './MenuSeparator.svelte';
	import SubMenu from './SubMenu.svelte';

	interface Props {
		items: MenuItemType[];
		parentRect: DOMRect;
		onclose: () => void;
		onaction: (item: MenuItemType) => void;
	}
	let { items, parentRect, onclose, onaction }: Props = $props();

	let menuEl = $state<HTMLDivElement | null>(null);
	let focusedIndex = $state(-1);
	let flipLeft = $state(false);
	let shiftUp = $state(0);

	let activeChildId = $state<string | null>(null);
	let childParentRect = $state<DOMRect | null>(null);
	let childItems = $state<MenuItemType[]>([]);
	let hoverTimer: ReturnType<typeof setTimeout> | null = null;
	let closeTimer: ReturnType<typeof setTimeout> | null = null;

	$effect(() => {
		if (!menuEl) return;
		const rect = menuEl.getBoundingClientRect();
		flipLeft = parentRect.right + rect.width > window.innerWidth - 8;
		const overflow = parentRect.top + rect.height - window.innerHeight + 8;
		shiftUp = overflow > 0 ? overflow : 0;
	});

	function clearTimers() {
		if (hoverTimer) {
			clearTimeout(hoverTimer);
			hoverTimer = null;
		}
		if (closeTimer) {
			clearTimeout(closeTimer);
			closeTimer = null;
		}
	}

	function handleItemHover(item: MenuItemType, el: HTMLElement) {
		clearTimers();
		if (item.kind === 'submenu') {
			hoverTimer = setTimeout(() => {
				activeChildId = item.id;
				childParentRect = el.getBoundingClientRect();
				childItems = item.children;
			}, 150);
		} else if (activeChildId) {
			closeTimer = setTimeout(() => {
				activeChildId = null;
				childItems = [];
				childParentRect = null;
			}, 100);
		}
	}

	function focusableIndices(): number[] {
		return items.map((item, i) => (item.kind !== 'separator' ? i : -1)).filter((i) => i >= 0);
	}

	function handleKeydown(e: KeyboardEvent) {
		const indices = focusableIndices();
		if (indices.length === 0) return;

		if (e.key === 'ArrowDown') {
			e.preventDefault();
			const pos = indices.indexOf(focusedIndex);
			focusedIndex = pos === -1 ? indices[0] : indices[(pos + 1) % indices.length];
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			const pos = indices.indexOf(focusedIndex);
			focusedIndex =
				pos === -1
					? indices[indices.length - 1]
					: indices[(pos - 1 + indices.length) % indices.length];
		} else if (e.key === 'ArrowRight') {
			e.preventDefault();
			if (focusedIndex >= 0 && focusedIndex < items.length) {
				const item = items[focusedIndex];
				if (item.kind === 'submenu') {
					const els = menuEl?.querySelectorAll('[role="menuitem"]');
					const menuitemIndex =
						items.filter((it, j) => j <= focusedIndex && it.kind !== 'separator').length - 1;
					const el = els?.[menuitemIndex] as HTMLElement | undefined;
					if (el) {
						activeChildId = item.id;
						childParentRect = el.getBoundingClientRect();
						childItems = item.children;
					}
				}
			}
		} else if (e.key === 'ArrowLeft' || e.key === 'Escape') {
			e.preventDefault();
			e.stopPropagation();
			if (activeChildId) {
				activeChildId = null;
				childItems = [];
				childParentRect = null;
			} else {
				onclose();
			}
		} else if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			if (focusedIndex >= 0 && focusedIndex < items.length) {
				const item = items[focusedIndex];
				if (item.kind === 'action' && !item.disabled) {
					onaction(item);
				} else if (item.kind === 'submenu') {
					const els = menuEl?.querySelectorAll('[role="menuitem"]');
					const menuitemIndex =
						items.filter((it, j) => j <= focusedIndex && it.kind !== 'separator').length - 1;
					const el = els?.[menuitemIndex] as HTMLElement | undefined;
					if (el) {
						activeChildId = item.id;
						childParentRect = el.getBoundingClientRect();
						childItems = item.children;
					}
				}
			}
		} else if (e.key.length === 1) {
			const char = e.key.toLowerCase();
			const match = indices.find(
				(i) =>
					items[i].kind !== 'separator' &&
					'label' in items[i] &&
					items[i].label.toLowerCase().startsWith(char)
			);
			if (match !== undefined) focusedIndex = match;
		}
	}
</script>

<div
	bind:this={menuEl}
	class="fixed z-50 min-w-[180px] max-w-[280px] rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-1.5 shadow-lg shadow-black/25"
	style="top: {parentRect.top - shiftUp}px; {flipLeft
		? `right: ${window.innerWidth - parentRect.left + 2}px`
		: `left: ${parentRect.right - 2}px`}"
	role="menu"
	tabindex="-1"
	onkeydown={handleKeydown}
>
	{#each items as item, i (item.kind === 'separator' ? `sep-${i}` : item.id)}
		{#if item.kind === 'separator'}
			<MenuSeparator />
		{:else}
			<MenuItem
				{item}
				focused={focusedIndex === i}
				onactivate={() => {
					if (item.kind === 'action' && !item.disabled) {
						onaction(item);
					} else if (item.kind === 'submenu') {
						const els = menuEl?.querySelectorAll('[role="menuitem"]');
						const menuitemIndex =
							items.filter((it, j) => j <= i && it.kind !== 'separator').length - 1;
						const el = els?.[menuitemIndex] as HTMLElement | undefined;
						if (el) {
							activeChildId = item.id;
							childParentRect = el.getBoundingClientRect();
							childItems = item.children;
						}
					}
				}}
				onhover={() => {
					focusedIndex = i;
					const els = menuEl?.querySelectorAll('[role="menuitem"]');
					const menuitemIndex =
						items.filter((it, j) => j <= i && it.kind !== 'separator').length - 1;
					const el = els?.[menuitemIndex] as HTMLElement | undefined;
					if (el) handleItemHover(item, el);
				}}
				onleave={() => {
					if (focusedIndex === i) focusedIndex = -1;
				}}
			/>
		{/if}
	{/each}
</div>

{#if activeChildId && childParentRect && childItems.length > 0}
	<SubMenu
		items={childItems}
		parentRect={childParentRect}
		onclose={() => {
			activeChildId = null;
			childItems = [];
			childParentRect = null;
		}}
		{onaction}
	/>
{/if}
