<script lang="ts">
	import type { PluginReportView } from '$lib/api/types';
	import { classificationClasses } from './styles';
	import ScoreBadge from './ScoreBadge.svelte';

	interface Props {
		plugins: PluginReportView[];
		onpluginclick?: (plugin: PluginReportView) => void;
	}

	let { plugins, onpluginclick }: Props = $props();

	const tally = $derived.by(() => {
		// eslint-disable-next-line svelte/prefer-svelte-reactivity -- local computation variable
		const counts = new Map<string, number>();
		for (const p of plugins) {
			if (p.report?.verdict?.classification) {
				const c = p.report.verdict.classification;
				counts.set(c, (counts.get(c) ?? 0) + 1);
			}
		}
		return Array.from(counts.entries())
			.sort(([a], [b]) => {
				const order = ['malicious', 'suspicious', 'unknown', 'clean'];
				return order.indexOf(a) - order.indexOf(b);
			})
			.map(([classification, count]) => ({ classification, count }));
	});

	let popoverOpen = $state(false);

	function displayName(p: PluginReportView): string {
		return p.report?.plugin.display_name ?? p.report?.plugin.id ?? p.plugin_name;
	}
</script>

<div class="flex items-center gap-2">
	{#each tally as entry (entry.classification)}
		<span
			class="rounded-full px-2.5 py-1 text-xs font-medium leading-none {classificationClasses(
				entry.classification
			).pill}"
		>
			{entry.count}
			{entry.classification}
		</span>
	{/each}

	<div class="relative">
		<button
			type="button"
			class="rounded-md border border-[var(--color-border)] bg-white/5 px-2 py-1 text-[11px] text-[var(--color-text-secondary)] transition-colors hover:bg-white/10"
			onclick={() => (popoverOpen = !popoverOpen)}
		>
			{plugins.length} plugin{plugins.length === 1 ? '' : 's'}
		</button>

		{#if popoverOpen}
			<button
				type="button"
				class="fixed inset-0 z-10 cursor-default"
				tabindex="-1"
				aria-label="Close popover"
				onclick={() => (popoverOpen = false)}
			></button>
			<div
				class="absolute right-0 top-full z-20 mt-1.5 min-w-[240px] rounded-lg border border-[var(--color-border)] bg-[#1e1f25] shadow-lg"
			>
				{#each plugins as plugin (plugin.plugin_name)}
					{@const cls = classificationClasses(plugin.report?.verdict?.classification)}
					<button
						type="button"
						class="flex w-full items-center gap-2.5 px-3 py-2 text-left text-xs transition-colors first:rounded-t-lg last:rounded-b-lg hover:bg-white/5"
						onclick={() => {
							popoverOpen = false;
							onpluginclick?.(plugin);
						}}
					>
						<span class="size-1.5 shrink-0 rounded-full {cls.dot}"></span>
						<span class="min-w-0 truncate text-[var(--color-text-primary)]">
							{displayName(plugin)}
						</span>
						{#if plugin.report?.verdict?.score != null}
							<span class="ml-auto shrink-0">
								<ScoreBadge
									score={plugin.report.verdict.score}
									classification={plugin.report.verdict.classification}
								/>
							</span>
						{/if}
					</button>
				{/each}
			</div>
		{/if}
	</div>
</div>
