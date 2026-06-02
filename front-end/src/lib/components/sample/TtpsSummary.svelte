<script lang="ts">
	import type { Ttp } from '$lib/api/types';

	interface Props {
		ttps: Ttp[];
	}

	let { ttps }: Props = $props();
</script>

{#if ttps.length > 0}
	<div class="space-y-3 rounded-xl bg-[var(--color-bg-secondary)] p-6">
		<h2 class="text-sm font-semibold uppercase tracking-wide text-[var(--color-text-secondary)]">
			MITRE ATT&CK TTPs ({ttps.length})
		</h2>

		<div class="space-y-1.5">
			{#each ttps as ttp (ttp.id)}
				<div
					class="flex items-center gap-3 rounded-lg bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm"
				>
					<span
						class="shrink-0 rounded bg-emerald-500/10 px-2 py-0.5 text-xs font-medium text-emerald-400"
					>
						{ttp.id}
					</span>
					<span class="text-[var(--color-text-primary)]">{ttp.name}</span>
					{#if ttp.evidence}
						<span class="text-xs text-[var(--color-text-secondary)]">
							{ttp.evidence}
						</span>
					{/if}
				</div>
			{/each}
		</div>
	</div>
{/if}
