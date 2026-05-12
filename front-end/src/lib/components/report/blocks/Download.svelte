<script lang="ts">
	import { resolveArtifactUrl } from '../artifact';
	import type { ArtifactLink } from '$lib/api/types';

	interface Props {
		artifact: string;
		label: string;
		artifacts: ArtifactLink[];
	}
	let { artifact, label, artifacts }: Props = $props();
	const url = $derived(resolveArtifactUrl(artifact, artifacts));
</script>

{#if url}
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
{:else}
	<div
		class="rounded-lg border-l-2 border-amber-500 bg-amber-500/10 p-3 text-xs text-amber-200"
	>
		Missing artifact: <span class="font-mono">{artifact}</span>
	</div>
{/if}
