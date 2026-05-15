<script lang="ts">
	import { SvelteSet } from 'svelte/reactivity';
	import { formatBytes } from '$lib/api/format';
	import { previewKind } from './artifact';
	import ArtifactPreview from './ArtifactPreview.svelte';
	import type { ArtifactLink } from '$lib/api/types';

	interface Props {
		artifacts: ArtifactLink[];
	}

	let { artifacts }: Props = $props();

	let previewArtifact = $state<ArtifactLink | null>(null);

	type ArtifactNode = {
		name: string;
		artifact?: ArtifactLink;
		children: Map<string, ArtifactNode>;
	};

	function fileName(path: string): string {
		const sep = path.lastIndexOf('/');
		const bsep = path.lastIndexOf('\\');
		const idx = Math.max(sep, bsep);
		return idx >= 0 ? path.slice(idx + 1) : path;
	}

	function splitPath(path: string): string[] {
		return path.split(/[/\\]/).filter(Boolean);
	}

	const tree = $derived.by(() => {
		const root: ArtifactNode = { name: '', children: new Map() };

		for (const a of artifacts) {
			const parts = splitPath(a.result_name);
			let node = root;
			for (let i = 0; i < parts.length - 1; i++) {
				const part = parts[i];
				if (!node.children.has(part)) {
					node.children.set(part, { name: part, children: new Map() });
				}
				node = node.children.get(part)!;
			}
			const leaf = parts[parts.length - 1];
			const existing = node.children.get(leaf);
			if (existing) {
				existing.artifact = a;
			} else {
				node.children.set(leaf, { name: leaf, artifact: a, children: new Map() });
			}
		}

		return root;
	});

	const topLevelFiles = $derived(
		[...tree.children.values()].filter((n) => n.artifact && n.children.size === 0)
	);
	const folders = $derived(
		[...tree.children.values()].filter((n) => n.children.size > 0 || !n.artifact)
	);

	let expandedFolders = new SvelteSet<string>();

	function toggleFolder(path: string) {
		if (expandedFolders.has(path)) {
			expandedFolders.delete(path);
		} else {
			expandedFolders.add(path);
		}
	}

	function collectFiles(node: ArtifactNode): ArtifactLink[] {
		const files: ArtifactLink[] = [];
		if (node.artifact) files.push(node.artifact);
		for (const child of node.children.values()) {
			files.push(...collectFiles(child));
		}
		return files;
	}

	function flattenFolder(
		node: ArtifactNode,
		prefix: string
	): { path: string; files: ArtifactLink[] } {
		let current = node;
		let fullPath = prefix ? `${prefix}/${node.name}` : node.name;

		while (current.children.size === 1 && !current.artifact) {
			const only = [...current.children.values()][0];
			if (only.artifact && only.children.size === 0) break;
			if (only.children.size === 0 && !only.artifact) break;
			current = only;
			fullPath = `${fullPath}/${current.name}`;
		}

		return { path: fullPath, files: collectFiles(current) };
	}
</script>

{#if artifacts.length > 0}
	<div class="space-y-2">
		{#if topLevelFiles.length > 0}
			<div class="flex flex-wrap gap-2">
				{#each topLevelFiles as node (node.name)}
					{@const a = node.artifact!}
					{@const canPreview = previewKind(a.result_name, a.format) !== 'none'}
					<div
						class="inline-flex items-center gap-2 rounded-lg bg-[var(--color-bg-card)] px-3 py-2 text-xs text-[var(--color-text-primary)]"
					>
						<span>{node.name}</span>
						<span class="text-[var(--color-text-secondary)]">&middot;</span>
						<span class="uppercase text-[var(--color-text-secondary)]">{a.format}</span>
						<span class="text-[var(--color-text-secondary)]">&middot;</span>
						<span class="text-[var(--color-text-secondary)]">{formatBytes(a.size_bytes)}</span>
						{#if canPreview}
							<button
								type="button"
								onclick={() => (previewArtifact = a)}
								class="ml-1 text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-accent)]"
								title="Preview"
							>
								<svg
									xmlns="http://www.w3.org/2000/svg"
									viewBox="0 0 20 20"
									fill="currentColor"
									class="size-3.5"
								>
									<path d="M10 12.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5Z" />
									<path
										fill-rule="evenodd"
										d="M.664 10.59a1.651 1.651 0 0 1 0-1.186A10.004 10.004 0 0 1 10 3c4.257 0 7.893 2.66 9.336 6.41.147.381.146.804 0 1.186A10.004 10.004 0 0 1 10 17c-4.257 0-7.893-2.66-9.336-6.41ZM14 10a4 4 0 1 1-8 0 4 4 0 0 1 8 0Z"
										clip-rule="evenodd"
									/>
								</svg>
							</button>
						{/if}
						<a
							href={a.url}
							download={a.result_name}
							class="text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-accent)]"
							title="Download"
						>
							&darr;
						</a>
					</div>
				{/each}
			</div>
		{/if}

		{#each folders as folder (folder.name)}
			{@const flat = flattenFolder(folder, '')}
			{@const expanded = expandedFolders.has(flat.path)}
			<div class="rounded-lg bg-[var(--color-bg-card)]">
				<button
					type="button"
					class="flex w-full items-center gap-2 px-3 py-2.5 text-xs transition-colors hover:bg-[var(--color-bg-tertiary)]"
					onclick={() => toggleFolder(flat.path)}
				>
					<svg
						xmlns="http://www.w3.org/2000/svg"
						viewBox="0 0 20 20"
						fill="currentColor"
						class="size-4 shrink-0 text-[var(--color-text-secondary)] transition-transform duration-150 {expanded
							? 'rotate-90'
							: ''}"
					>
						<path
							fill-rule="evenodd"
							d="M7.21 14.77a.75.75 0 0 1 .02-1.06L11.168 10 7.23 6.29a.75.75 0 1 1 1.04-1.08l4.5 4.25a.75.75 0 0 1 0 1.08l-4.5 4.25a.75.75 0 0 1-1.06-.02Z"
							clip-rule="evenodd"
						/>
					</svg>
					<svg
						xmlns="http://www.w3.org/2000/svg"
						viewBox="0 0 20 20"
						fill="currentColor"
						class="size-4 shrink-0 text-[var(--color-accent)]"
					>
						<path
							d="M3.75 3A1.75 1.75 0 0 0 2 4.75v3.26a3.235 3.235 0 0 1 1.75-.51h12.5c.644 0 1.245.188 1.75.51V6.75A1.75 1.75 0 0 0 16.25 5h-4.836a.25.25 0 0 1-.177-.073L9.823 3.513A1.75 1.75 0 0 0 8.586 3H3.75Z"
						/>
						<path
							d="M3.75 9A1.75 1.75 0 0 0 2 10.75v4.5c0 .966.784 1.75 1.75 1.75h12.5A1.75 1.75 0 0 0 18 15.25v-4.5A1.75 1.75 0 0 0 16.25 9H3.75Z"
						/>
					</svg>
					<span class="truncate text-[var(--color-text-primary)]">{flat.path}</span>
					<span class="ml-auto shrink-0 text-[var(--color-text-secondary)]">
						{flat.files.length} file{flat.files.length === 1 ? '' : 's'}
					</span>
				</button>

				{#if expanded}
					<div class="max-h-80 overflow-y-auto border-t border-[var(--color-border)]/50">
						{#each flat.files as a (a.result_name)}
							{@const canPreview = previewKind(a.result_name, a.format) !== 'none'}
							<div
								class="flex items-center gap-2 px-3 py-2 text-xs transition-colors hover:bg-[var(--color-bg-tertiary)]"
							>
								<span class="w-4"></span>
								<span class="truncate text-[var(--color-text-primary)]" title={a.result_name}>
									{fileName(a.result_name)}
								</span>
								<span
									class="ml-auto flex shrink-0 items-center gap-2 text-[var(--color-text-secondary)]"
								>
									<span class="uppercase">{a.format}</span>
									<span>&middot;</span>
									<span>{formatBytes(a.size_bytes)}</span>
									{#if canPreview}
										<button
											type="button"
											onclick={() => (previewArtifact = a)}
											class="text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-accent)]"
											title="Preview"
										>
											<svg
												xmlns="http://www.w3.org/2000/svg"
												viewBox="0 0 20 20"
												fill="currentColor"
												class="size-3.5"
											>
												<path d="M10 12.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5Z" />
												<path
													fill-rule="evenodd"
													d="M.664 10.59a1.651 1.651 0 0 1 0-1.186A10.004 10.004 0 0 1 10 3c4.257 0 7.893 2.66 9.336 6.41.147.381.146.804 0 1.186A10.004 10.004 0 0 1 10 17c-4.257 0-7.893-2.66-9.336-6.41ZM14 10a4 4 0 1 1-8 0 4 4 0 0 1 8 0Z"
													clip-rule="evenodd"
												/>
											</svg>
										</button>
									{/if}
									<a
										href={a.url}
										download={a.result_name}
										class="text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-accent)]"
										title="Download"
									>
										&darr;
									</a>
								</span>
							</div>
						{/each}
					</div>
				{/if}
			</div>
		{/each}
	</div>

	<ArtifactPreview artifact={previewArtifact} onclose={() => (previewArtifact = null)} />
{/if}
