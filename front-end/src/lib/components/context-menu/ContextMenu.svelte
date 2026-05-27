<script lang="ts">
	import { contextMenuStore } from '$lib/stores/contextMenu.svelte';
	import type { MenuItem as MenuItemType } from './types';
	import MenuItem from './MenuItem.svelte';
	import MenuSeparator from './MenuSeparator.svelte';
	import SubMenu from './SubMenu.svelte';

	let menuEl = $state<HTMLDivElement | null>(null);
	let submenuParentEl = $state<HTMLElement | null>(null);
	let submenuParentRect = $state<DOMRect | null>(null);
	let submenuItems = $state<MenuItemType[]>([]);

	let hoverTimer: ReturnType<typeof setTimeout> | null = null;
	let closeTimer: ReturnType<typeof setTimeout> | null = null;

	let safeTriangleOrigin = $state<{ x: number; y: number } | null>(null);

	const adjustedPosition = $derived.by(() => {
		if (!menuEl || !contextMenuStore.open) return contextMenuStore.position;
		const rect = menuEl.getBoundingClientRect();
		let { x, y } = contextMenuStore.position;
		if (x + rect.width > window.innerWidth - 8) x = window.innerWidth - rect.width - 8;
		if (y + rect.height > window.innerHeight - 8) y = window.innerHeight - rect.height - 8;
		if (x < 8) x = 8;
		if (y < 8) y = 8;
		return { x, y };
	});

	function closeAll() {
		contextMenuStore.closeMenu();
		clearTimers();
		submenuItems = [];
		submenuParentRect = null;
		submenuParentEl = null;
		safeTriangleOrigin = null;
	}

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
			safeTriangleOrigin = null;
			hoverTimer = setTimeout(() => {
				contextMenuStore.setActiveSubmenu(item.id);
				submenuParentEl = el;
				submenuParentRect = el.getBoundingClientRect();
				submenuItems = item.children;
			}, 150);
		} else {
			if (contextMenuStore.activeSubmenu) {
				closeTimer = setTimeout(() => {
					contextMenuStore.setActiveSubmenu(null);
					submenuItems = [];
					submenuParentRect = null;
				}, 100);
			}
		}
	}

	function handleItemLeave(e: PointerEvent) {
		if (contextMenuStore.activeSubmenu && submenuParentRect) {
			safeTriangleOrigin = { x: e.clientX, y: e.clientY };
		}
	}

	function isInsideSafeTriangle(mx: number, my: number): boolean {
		if (!safeTriangleOrigin || !submenuParentRect) return false;
		const { x: ox, y: oy } = safeTriangleOrigin;
		const topRight = { x: submenuParentRect.right, y: submenuParentRect.top };
		const bottomRight = { x: submenuParentRect.right, y: submenuParentRect.bottom };
		return pointInTriangle(mx, my, ox, oy, topRight.x, topRight.y, bottomRight.x, bottomRight.y);
	}

	function pointInTriangle(
		px: number,
		py: number,
		ax: number,
		ay: number,
		bx: number,
		by: number,
		cx: number,
		cy: number
	): boolean {
		const d1 = sign(px, py, ax, ay, bx, by);
		const d2 = sign(px, py, bx, by, cx, cy);
		const d3 = sign(px, py, cx, cy, ax, ay);
		const hasNeg = d1 < 0 || d2 < 0 || d3 < 0;
		const hasPos = d1 > 0 || d2 > 0 || d3 > 0;
		return !(hasNeg && hasPos);
	}

	function sign(px: number, py: number, x1: number, y1: number, x2: number, y2: number): number {
		return (px - x2) * (y1 - y2) - (x1 - x2) * (py - y2);
	}

	function handleMenuMouseMove(e: MouseEvent) {
		if (safeTriangleOrigin && !isInsideSafeTriangle(e.clientX, e.clientY)) {
			safeTriangleOrigin = null;
			contextMenuStore.setActiveSubmenu(null);
			submenuItems = [];
			submenuParentRect = null;
		}
	}

	function handleAction(item: MenuItemType) {
		if (item.kind === 'action' && !item.disabled && contextMenuStore.context) {
			item.handler(contextMenuStore.context);
		}
		closeAll();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (!contextMenuStore.open) return;

		if (e.key === 'Escape') {
			e.preventDefault();
			if (contextMenuStore.activeSubmenu) {
				contextMenuStore.setActiveSubmenu(null);
				submenuItems = [];
				submenuParentRect = null;
			} else {
				closeAll();
			}
			return;
		}

		if (e.key === 'ArrowDown') {
			e.preventDefault();
			contextMenuStore.moveFocus('down');
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			contextMenuStore.moveFocus('up');
		} else if (e.key === 'ArrowRight') {
			e.preventDefault();
			const focused = contextMenuStore.getFocusedItem();
			if (focused?.kind === 'submenu') {
				contextMenuStore.setActiveSubmenu(focused.id);
				const idx = contextMenuStore.items.indexOf(focused);
				const els = menuEl?.querySelectorAll('[role="menuitem"]');
				if (els?.[idx]) {
					submenuParentEl = els[idx] as HTMLElement;
					submenuParentRect = submenuParentEl.getBoundingClientRect();
					submenuItems = focused.children;
				}
			}
		} else if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			const focused = contextMenuStore.getFocusedItem();
			if (focused) handleAction(focused);
		} else if (e.key.length === 1) {
			const char = e.key.toLowerCase();
			const match = contextMenuStore.items.findIndex(
				(it) => it.kind !== 'separator' && it.label.toLowerCase().startsWith(char)
			);
			if (match >= 0) contextMenuStore.focusedIndex = match;
		}
	}

	$effect(() => {
		if (!contextMenuStore.open) return;

		function onScroll() {
			closeAll();
		}

		document.addEventListener('keydown', handleKeydown);
		window.addEventListener('scroll', onScroll, true);

		return () => {
			document.removeEventListener('keydown', handleKeydown);
			window.removeEventListener('scroll', onScroll, true);
			clearTimers();
		};
	});
</script>

{#if contextMenuStore.open}
	<!-- Backdrop -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<div
		class="fixed inset-0 z-40"
		onclick={closeAll}
		oncontextmenu={(e) => {
			e.preventDefault();
			closeAll();
		}}
	></div>

	<!-- Menu -->
	<div
		bind:this={menuEl}
		class="fixed z-50 min-w-[200px] max-w-[280px] rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-1.5 shadow-lg shadow-black/25"
		style="left: {adjustedPosition.x}px; top: {adjustedPosition.y}px"
		role="menu"
		tabindex="-1"
		onpointermove={handleMenuMouseMove}
	>
		{#each contextMenuStore.items as item, i (item.kind === 'separator' ? `sep-${i}` : item.id)}
			{#if item.kind === 'separator'}
				<MenuSeparator />
			{:else}
				<MenuItem
					{item}
					focused={contextMenuStore.focusedIndex === i}
					onactivate={() => {
						if (item.kind === 'submenu') {
							const el = menuEl?.querySelectorAll('[role="menuitem"]')[
								contextMenuStore.items.filter((it, j) => j <= i && it.kind !== 'separator').length -
									1
							] as HTMLElement | undefined;
							if (el) {
								contextMenuStore.setActiveSubmenu(item.id);
								submenuParentEl = el;
								submenuParentRect = el.getBoundingClientRect();
								submenuItems = item.children;
							}
						} else {
							handleAction(item);
						}
					}}
					onhover={() => {
						contextMenuStore.focusedIndex = i;
						const els = menuEl?.querySelectorAll('[role="menuitem"]');
						const menuitemIndex =
							contextMenuStore.items.filter((it, j) => j <= i && it.kind !== 'separator').length -
							1;
						const el = els?.[menuitemIndex] as HTMLElement | undefined;
						if (el) handleItemHover(item, el);
					}}
					onleave={(e) => handleItemLeave(e)}
				/>
			{/if}
		{/each}
	</div>

	<!-- Submenu -->
	{#if contextMenuStore.activeSubmenu && submenuParentRect && submenuItems.length > 0}
		<SubMenu
			items={submenuItems}
			parentRect={submenuParentRect}
			onclose={() => {
				contextMenuStore.setActiveSubmenu(null);
				submenuItems = [];
				submenuParentRect = null;
			}}
			onaction={handleAction}
		/>
	{/if}
{/if}
