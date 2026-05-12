export type ToastKind = 'info' | 'success' | 'warning' | 'error';

export interface Toast {
	id: number;
	kind: ToastKind;
	message: string;
	ttlMs: number;
}

class ToastStore {
	private items = $state<Toast[]>([]);
	private nextId = 1;

	get list(): Toast[] {
		return this.items;
	}

	push(toast: Omit<Toast, 'id' | 'ttlMs'>, ttlMs = 5000): number {
		const id = this.nextId++;
		this.items = [...this.items, { id, ttlMs, ...toast }];
		if (ttlMs > 0) setTimeout(() => this.dismiss(id), ttlMs);
		return id;
	}

	dismiss(id: number): void {
		this.items = this.items.filter((t) => t.id !== id);
	}

	clear(): void {
		this.items = [];
	}
}

export const toasts = new ToastStore();
