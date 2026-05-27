import type { TransformCategory, TransformDefinition, TransformProvenance } from './types';

const PRIORITY: Record<TransformProvenance, number> = {
	builtin: 0,
	'git-synced': 1,
	'ui-managed': 2
};

export class TransformRegistry {
	private transforms = new Map<string, TransformDefinition>();

	register(transform: TransformDefinition): void {
		const existing = this.transforms.get(transform.id);
		if (existing && PRIORITY[existing.provenance] > PRIORITY[transform.provenance]) {
			return;
		}
		this.transforms.set(transform.id, transform);
	}

	unregister(id: string): void {
		this.transforms.delete(id);
	}

	get(id: string): TransformDefinition | undefined {
		return this.transforms.get(id);
	}

	list(category?: TransformCategory): TransformDefinition[] {
		const all = Array.from(this.transforms.values());
		if (category) {
			return all.filter((t) => t.category === category);
		}
		return all;
	}

	listDetectable(): TransformDefinition[] {
		return Array.from(this.transforms.values()).filter((t) => typeof t.detect === 'function');
	}

	clear(): void {
		this.transforms.clear();
	}
}
