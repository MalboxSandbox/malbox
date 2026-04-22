<script lang="ts">
	import { formatBytes } from '$lib/api/format';
	import type { ArtifactLink } from '$lib/api/types';

	interface Props {
		artifacts: ArtifactLink[];
	}

	let { artifacts }: Props = $props();
</script>

{#if artifacts.length > 0}
	<div class="flex flex-wrap gap-2">
		{#each artifacts as a (a.result_name)}
			<a
				href={a.url}
				download={a.result_name}
				class="inline-flex items-center gap-2 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-card)] px-3 py-2 text-xs text-[var(--color-text-primary)] transition-colors hover:bg-[var(--color-bg-tertiary)]"
			>
				<span class="font-mono">{a.result_name}</span>
				<span class="text-[var(--color-text-secondary)]">·</span>
				<span class="uppercase text-[var(--color-text-secondary)]">{a.format}</span>
				<span class="text-[var(--color-text-secondary)]">·</span>
				<span class="text-[var(--color-text-secondary)]">{formatBytes(a.size_bytes)}</span>
				<span class="ml-1 text-[var(--color-accent)]">↓</span>
			</a>
		{/each}
	</div>
{/if}
