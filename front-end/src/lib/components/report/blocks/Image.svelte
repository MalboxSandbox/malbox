<script lang="ts">
	import { resolveArtifactUrl } from '../artifact';
	import type { ArtifactLink } from '$lib/api/types';

	interface Props {
		artifact: string;
		caption?: string;
		artifacts: ArtifactLink[];
	}
	let { artifact, caption, artifacts }: Props = $props();
	const url = $derived(resolveArtifactUrl(artifact, artifacts));
</script>

{#if url}
	<figure class="space-y-2">
		<img src={url} alt={caption ?? artifact} class="max-w-full rounded-lg" />
		{#if caption}
			<figcaption class="text-xs text-[var(--color-text-secondary)]">{caption}</figcaption>
		{/if}
	</figure>
{:else}
	<div class="rounded-lg border-l-2 border-amber-500 bg-amber-500/10 p-3 text-xs text-amber-200">
		Missing artifact: <span class="font-mono">{artifact}</span>
	</div>
{/if}
