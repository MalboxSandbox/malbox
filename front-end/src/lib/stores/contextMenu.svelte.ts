import type { ContextPayload, MenuItem } from '$lib/components/context-menu/types';

class ContextMenuStore {
	open = $state(false);
	position = $state({ x: 0, y: 0 });
	context = $state<ContextPayload | null>(null);
	items = $state<MenuItem[]>([]);
	activeSubmenu = $state<string | null>(null);
	focusedIndex = $state(-1);

	openMenu(pos: { x: number; y: number }, ctx: ContextPayload, menuItems: MenuItem[]): void {
		this.position = pos;
		this.context = ctx;
		this.items = menuItems;
		this.activeSubmenu = null;
		this.focusedIndex = -1;
		this.open = true;
	}

	closeMenu(): void {
		this.open = false;
		this.context = null;
		this.items = [];
		this.activeSubmenu = null;
		this.focusedIndex = -1;
	}

	setActiveSubmenu(id: string | null): void {
		this.activeSubmenu = id;
	}

	private focusableIndices(): number[] {
		return this.items.map((item, i) => (item.kind !== 'separator' ? i : -1)).filter((i) => i >= 0);
	}

	moveFocus(direction: 'up' | 'down'): void {
		const indices = this.focusableIndices();
		if (indices.length === 0) return;

		const currentPos = indices.indexOf(this.focusedIndex);
		if (currentPos === -1) {
			this.focusedIndex = direction === 'down' ? indices[0] : indices[indices.length - 1];
		} else if (direction === 'down') {
			this.focusedIndex = indices[(currentPos + 1) % indices.length];
		} else {
			this.focusedIndex = indices[(currentPos - 1 + indices.length) % indices.length];
		}
	}

	getFocusedItem(): MenuItem | null {
		if (this.focusedIndex < 0 || this.focusedIndex >= this.items.length) return null;
		return this.items[this.focusedIndex];
	}
}

export const contextMenuStore = new ContextMenuStore();
