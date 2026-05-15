<script lang="ts">
	import { resolveArtifactUrl, resolveArtifact, previewKind } from '../artifact';
	import ArtifactPreview from '../ArtifactPreview.svelte';
	import type { ArtifactLink } from '$lib/api/types';

	interface Props {
		artifact: string;
		label: string;
		artifacts: ArtifactLink[];
	}
	let { artifact, label, artifacts }: Props = $props();
	const url = $derived(resolveArtifactUrl(artifact, artifacts));
	const resolved = $derived(resolveArtifact(artifact, artifacts));
	const canPreview = $derived(
		resolved ? previewKind(resolved.result_name, resolved.format) !== 'none' : false
	);
	let previewTarget = $state<ArtifactLink | null>(null);
</script>

{#if url}
	<div class="flex flex-wrap items-center gap-2">
		{#if canPreview}
			<button
				type="button"
				onclick={() => (previewTarget = resolved)}
				class="inline-flex items-center gap-2 rounded-lg bg-[var(--color-bg-card)] px-4 py-2.5 text-sm text-[var(--color-text-primary)] transition-colors hover:bg-[var(--color-bg-tertiary)]"
			>
				<svg
					xmlns="http://www.w3.org/2000/svg"
					viewBox="0 0 20 20"
					fill="currentColor"
					class="size-4 text-[var(--color-accent)]"
				>
					<path d="M10 12.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5Z" />
					<path
						fill-rule="evenodd"
						d="M.664 10.59a1.651 1.651 0 0 1 0-1.186A10.004 10.004 0 0 1 10 3c4.257 0 7.893 2.66 9.336 6.41.147.381.146.804 0 1.186A10.004 10.004 0 0 1 10 17c-4.257 0-7.893-2.66-9.336-6.41ZM14 10a4 4 0 1 1-8 0 4 4 0 0 1 8 0Z"
						clip-rule="evenodd"
					/>
				</svg>
				<span>Preview</span>
			</button>
		{/if}
		<a
			href={url}
			download={artifact}
			class="inline-flex items-center gap-2 rounded-lg bg-[var(--color-bg-card)] px-4 py-2.5 text-sm text-[var(--color-text-primary)] transition-colors hover:bg-[var(--color-bg-tertiary)]"
		>
			<svg
				xmlns="http://www.w3.org/2000/svg"
				viewBox="0 0 20 20"
				fill="currentColor"
				class="size-4 text-[var(--color-accent)]"
			>
				<path
					d="M10.75 2.75a.75.75 0 0 0-1.5 0v8.614L6.295 8.235a.75.75 0 1 0-1.09 1.03l4.25 4.5a.75.75 0 0 0 1.09 0l4.25-4.5a.75.75 0 0 0-1.09-1.03l-2.955 3.129V2.75Z"
				/>
				<path
					d="M3.5 12.75a.75.75 0 0 0-1.5 0v2.5A2.75 2.75 0 0 0 4.75 18h10.5A2.75 2.75 0 0 0 18 15.25v-2.5a.75.75 0 0 0-1.5 0v2.5c0 .69-.56 1.25-1.25 1.25H4.75c-.69 0-1.25-.56-1.25-1.25v-2.5Z"
				/>
			</svg>
			<span>{label}</span>
		</a>
	</div>
	<ArtifactPreview artifact={previewTarget} onclose={() => (previewTarget = null)} />
{:else}
	<div class="rounded-lg border-l-2 border-amber-500 bg-amber-500/10 p-3 text-xs text-amber-200">
		Missing artifact: <span class="font-mono">{artifact}</span>
	</div>
{/if}
