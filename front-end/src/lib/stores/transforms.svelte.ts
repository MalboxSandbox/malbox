import { browser } from '$app/environment';
import {
	TransformRegistry,
	AutoDetect,
	Pipeline,
	registerBuiltins,
	loadYamlTransform,
	loadJsTransform,
	loadWasmTransform
} from '$lib/transforms';
import { runInSandbox } from '$lib/transforms/engines/sandbox';
import { listCustomTransforms } from '$lib/api/transforms';
import type { TransformDefinition, TransformProvenance } from '$lib/transforms';
import type { CustomTransform } from '$lib/api/types';

class TransformRegistryStore {
	registry = $state(new TransformRegistry());
	autoDetect = $derived(new AutoDetect(this.registry));
	pipeline = $derived(new Pipeline(this.registry));
	loaded = $state(false);

	constructor() {
		registerBuiltins(this.registry);
	}

	get transforms(): TransformDefinition[] {
		return this.registry.list();
	}

	get categories(): string[] {
		const cats = new Set(this.registry.list().map((t) => t.category));
		return Array.from(cats).sort();
	}

	async loadCustomTransforms(fetchFn: typeof fetch): Promise<void> {
		if (!browser) return;
		try {
			const customs = await listCustomTransforms(fetchFn);
			for (const ct of customs) {
				this.loadCustomTransform(ct);
			}
		} catch {
			// API may not be available yet
		} finally {
			this.loaded = true;
		}
	}

	private sandboxJsTransform(
		content: string,
		id: string,
		name: string,
		category: string,
		provenance: TransformProvenance
	): TransformDefinition {
		const functionBody = `
			const mod = (${content});
			const result = await mod.apply(input, params);
			return result;
		`;
		return {
			id,
			name,
			category: category as TransformDefinition['category'],
			provenance,
			async apply(input, params) {
				return runInSandbox(functionBody, input, params);
			}
		};
	}

	private async loadCustomTransform(ct: CustomTransform): Promise<void> {
		const provenance = ct.git_synced ? ('git-synced' as const) : ('ui-managed' as const);
		try {
			if (ct.kind === 'yaml') {
				const transform = loadYamlTransform(ct.content, this.registry, provenance);
				this.registry.register(transform);
			} else if (ct.kind === 'js') {
				if (browser) {
					this.registry.register(
						this.sandboxJsTransform(ct.content, ct.transform_id, ct.name, ct.category, provenance)
					);
				} else {
					const module = new Function(`return (${ct.content})`)();
					const transform = loadJsTransform(module, provenance);
					this.registry.register(transform);
				}
			} else if (ct.kind === 'wasm') {
				const wrapper = new Function(`return (${ct.content})`)();
				const transform = await loadWasmTransform(wrapper, provenance);
				this.registry.register(transform);
			}
		} catch (e) {
			console.warn(`Failed to load custom transform "${ct.transform_id}":`, e);
		}
	}
}

export const transformStore = new TransformRegistryStore();
