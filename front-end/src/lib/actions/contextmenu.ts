import type { ContextPayload } from '$lib/components/context-menu/types';
import { contextMenuStore } from '$lib/stores/contextMenu.svelte';
import { resolveMenuItems } from '$lib/context-menu/registry';

type ContextMenuParam = (ContextPayload & { selectionOnly?: boolean }) | undefined;

export function contextmenu(node: HTMLElement, param: ContextMenuParam) {
	let currentParam = param;

	function handleContextMenu(e: MouseEvent) {
		// No payload means this element opts out: let the native menu through.
		if (!currentParam) return;

		const selection = window.getSelection()?.toString().trim();

		if (currentParam.selectionOnly && !selection) return;

		e.preventDefault();
		e.stopPropagation();

		const { selectionOnly: _, ...payload } = currentParam;
		let effective: ContextPayload = payload;
		if (selection) {
			effective = { ...payload, value: selection };
		}

		const items = resolveMenuItems(effective);
		if (items.length === 0) return;

		contextMenuStore.openMenu({ x: e.clientX, y: e.clientY }, effective, items);
	}

	node.addEventListener('contextmenu', handleContextMenu);

	return {
		update(newParam: ContextMenuParam) {
			currentParam = newParam;
		},
		destroy() {
			node.removeEventListener('contextmenu', handleContextMenu);
		}
	};
}
