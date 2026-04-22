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
		class="inline-flex items-center gap-2 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-card)] px-4 py-2 text-sm text-[var(--color-text-primary)] hover:bg-[var(--color-bg-tertiary)]"
	>
		<span>{label}</span>
		<span class="text-[var(--color-accent)]">↓</span>
	</a>
{:else}
	<div
		class="rounded-lg border border-amber-500/40 bg-amber-500/10 p-3 text-xs text-amber-200"
	>
		Missing artifact: <span class="font-mono">{artifact}</span>
	</div>
{/if}
