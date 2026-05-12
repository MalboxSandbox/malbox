<script lang="ts">
	import ScoreBar from './ScoreBar.svelte';
	import ArtifactLinks from './ArtifactLinks.svelte';
	import Section from './Section.svelte';
	import type { PluginReportView } from '$lib/api/types';

	interface Props {
		view: PluginReportView;
	}
	let { view }: Props = $props();

	const report = $derived(view.report);
	const schemaWarning = $derived(report && report.schema_version > 1);
	const displayName = $derived(
		report?.plugin.display_name ?? report?.plugin.id ?? view.plugin_name
	);

	const artifactNames = $derived(new Set(view.artifacts.map((a) => a.result_name)));

	function isArtifactSection(s: { title: string; blocks?: { type: string }[] }): boolean {
		if (s.blocks && s.blocks.length === 1) {
			const t = s.blocks[0].type;
			if (t === 'download' || t === 'json') return true;
		}
		return artifactNames.has(s.title);
	}

	const contentSections = $derived(
		report?.sections?.filter((s) => !view.synthesized || !isArtifactSection(s)) ?? []
	);
</script>

<div class="space-y-6">
	<div class="space-y-4 rounded-2xl bg-[var(--color-bg-secondary)] p-8">
		<div class="flex flex-wrap items-start justify-between gap-4">
			<div class="space-y-1">
				<h1 class="text-2xl font-semibold text-[var(--color-text-primary)]">
					{displayName}
				</h1>
				{#if report?.plugin}
					<div class="text-xs text-[var(--color-text-secondary)]">
						<span class="font-mono">{report.plugin.id}</span>
						{#if report.plugin.version}
							<span>· v{report.plugin.version}</span>
						{/if}
					</div>
				{/if}
			</div>
			{#if report?.verdict?.score !== undefined}
				<ScoreBar
					score={report.verdict.score}
					classification={report.verdict.classification}
				/>
			{/if}
		</div>

		{#if report?.summary}
			<p class="text-sm text-[var(--color-text-primary)]">{report.summary}</p>
		{/if}

		<div class="flex flex-wrap gap-2">
			{#if view.synthesized}
				<span
					class="rounded bg-[var(--color-bg-tertiary)] px-2 py-0.5 text-xs text-[var(--color-text-secondary)]"
				>
					synthesized
				</span>
			{/if}
			{#if report?.verdict?.labels}
				{#each report.verdict.labels as l (l)}
					<span
						class="rounded bg-[var(--color-bg-tertiary)] px-2 py-0.5 text-xs text-[var(--color-text-secondary)]"
					>
						{l}
					</span>
				{/each}
			{/if}
		</div>

		{#if schemaWarning}
			<div class="rounded-lg border-l-2 border-amber-500 bg-amber-500/10 p-3 text-xs text-amber-200">
				This report uses schema version {report?.schema_version}; some fields may not render.
			</div>
		{/if}
	</div>

	{#if view.artifacts.length > 0}
		<div class="space-y-3 rounded-2xl bg-[var(--color-bg-secondary)] p-8">
			<h2 class="text-sm font-medium text-[var(--color-text-secondary)]">Artifacts</h2>
			<ArtifactLinks artifacts={view.artifacts} />
		</div>
	{/if}

	{#if !report}
		<div class="rounded-2xl bg-[var(--color-bg-secondary)] p-8 text-sm text-[var(--color-text-secondary)]">
			This plugin did not produce a readable report.
		</div>
	{:else if contentSections.length > 0}
		{#each contentSections as s (s.id)}
			<Section section={s} artifacts={view.artifacts} />
		{/each}
	{/if}
</div>
