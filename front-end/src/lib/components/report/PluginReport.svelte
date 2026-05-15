<script lang="ts">
	import ScoreBadge from './ScoreBadge.svelte';
	import Section from './Section.svelte';
	import BlockRenderer from './BlockRenderer.svelte';
	import ArtifactPreview from './ArtifactPreview.svelte';
	import { formatBytes } from '$lib/api/format';
	import { previewKind } from './artifact';
	import type { PluginReportView, Sample, KvPair, Block } from '$lib/api/types';

	interface Props {
		view: PluginReportView;
		sample?: Sample | null;
	}
	let { view, sample = null }: Props = $props();

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

	const overviewSection = $derived(
		contentSections.find((s) => s.title.toLowerCase() === 'overview')
	);

	const remainingSections = $derived(contentSections.filter((s) => s !== overviewSection));

	const sampleValues = $derived.by(() => {
		if (!sample) return new Set<string>();
		return new Set(
			[
				sample.md5,
				sample.sha1,
				sample.sha256,
				sample.sha512,
				sample.crc32,
				sample.ssdeep,
				sample.file_type
			].filter(Boolean)
		);
	});

	function isGenericPair(p: KvPair): boolean {
		if (!sample) return false;
		if (sampleValues.has(p.value)) return true;
		const digits = p.value.replace(/\D/g, '');
		if (digits && Number(digits) === sample.file_size) return true;
		return false;
	}

	const overviewKvPairs = $derived.by<KvPair[]>(() => {
		const pairs: KvPair[] = [];
		for (const b of overviewSection?.blocks ?? []) {
			if (b.type === 'kv') pairs.push(...b.pairs);
		}
		return pairs;
	});

	const findingPairs = $derived(overviewKvPairs.filter((p) => !isGenericPair(p)));

	const overviewTableBlocks = $derived.by<Block[]>(() => {
		return (overviewSection?.blocks ?? []).filter((b) => b.type === 'table');
	});

	const overviewOtherBlocks = $derived.by<Block[]>(() => {
		return (overviewSection?.blocks ?? []).filter((b) => b.type !== 'kv' && b.type !== 'table');
	});

	const hasTags = $derived(
		view.synthesized ||
			(report?.verdict?.labels && report.verdict.labels.length > 0) ||
			findingPairs.length > 0
	);

	function fileName(path: string): string {
		const idx = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
		return idx >= 0 ? path.slice(idx + 1) : path;
	}

	const pill =
		'rounded bg-[var(--color-bg-tertiary)] px-2 py-0.5 text-xs text-[var(--color-text-secondary)]';

	let previewArtifact = $state<(typeof view.artifacts)[number] | null>(null);
</script>

<div class="space-y-6">
	<div class="rounded-2xl bg-[var(--color-bg-secondary)]">
		<!-- Header -->
		<div class="p-8">
			<div class="flex items-start justify-between gap-4">
				<div class="min-w-0">
					<div class="flex items-baseline gap-2.5">
						<h1 class="text-2xl font-semibold text-[var(--color-text-primary)]">
							{displayName}
						</h1>
						{#if report?.plugin.version}
							<span class="text-sm text-[var(--color-text-secondary)]">
								v{report.plugin.version}
							</span>
						{/if}
					</div>
					{#if report?.summary}
						<p class="mt-2 text-sm text-[var(--color-text-secondary)]">
							{report.summary}
						</p>
					{/if}
				</div>
				<div class="shrink-0">
					<ScoreBadge
						score={report?.verdict?.score ?? 0}
						classification={report?.verdict?.classification}
					/>
				</div>
			</div>

			{#if hasTags}
				<div class="mt-4 flex flex-wrap items-center gap-2">
					{#if view.synthesized}
						<span class={pill}>synthesized</span>
					{/if}
					{#if report?.verdict?.labels}
						{#each report.verdict.labels as l (l)}
							<span class={pill}>{l}</span>
						{/each}
					{/if}
					{#each findingPairs as p (p.key)}
						<span class={pill}
							>{p.key}:
							<span class="text-[var(--color-text-primary)]">{p.value}</span></span
						>
					{/each}
				</div>
			{/if}

			{#if schemaWarning}
				<div
					class="mt-4 rounded-lg border-l-2 border-amber-500 bg-amber-500/10 p-3 text-xs text-amber-200"
				>
					This report uses schema version {report?.schema_version}; some fields may not render.
				</div>
			{/if}
		</div>

		<!-- Detection tables from overview -->
		{#if overviewTableBlocks.length > 0}
			<div class="space-y-4 border-t border-[var(--color-border)]/20 px-8 py-6">
				{#each overviewTableBlocks as block, i (i)}
					<BlockRenderer {block} artifacts={view.artifacts} />
				{/each}
			</div>
		{/if}

		<!-- Other overview blocks -->
		{#if overviewOtherBlocks.length > 0}
			<div class="space-y-4 border-t border-[var(--color-border)]/20 px-8 py-6">
				{#each overviewOtherBlocks as block, i (i)}
					<BlockRenderer {block} artifacts={view.artifacts} />
				{/each}
			</div>
		{/if}

		<!-- Artifacts -->
		{#if view.artifacts.length > 0}
			<div class="border-t border-[var(--color-border)]/20 px-8 py-6">
				<div class="space-y-0.5">
					{#each view.artifacts as a (a.result_name)}
						{@const canPreview = previewKind(a.result_name, a.format) !== 'none'}
						<div
							class="-mx-2 flex items-center gap-3 rounded-lg px-2 py-1.5 text-xs transition-colors hover:bg-[var(--color-bg-tertiary)]/40"
						>
							<svg
								xmlns="http://www.w3.org/2000/svg"
								viewBox="0 0 16 16"
								fill="currentColor"
								class="size-3.5 shrink-0 text-[var(--color-text-secondary)]"
							>
								<path
									fill-rule="evenodd"
									d="M4 2a1.5 1.5 0 0 0-1.5 1.5v9A1.5 1.5 0 0 0 4 14h8a1.5 1.5 0 0 0 1.5-1.5V6.621a1.5 1.5 0 0 0-.44-1.06L9.94 2.439A1.5 1.5 0 0 0 8.878 2H4Z"
									clip-rule="evenodd"
								/>
							</svg>
							<span class="min-w-0 truncate text-[var(--color-text-primary)]" title={a.result_name}>
								{fileName(a.result_name)}
							</span>
							<span
								class="shrink-0 rounded bg-[var(--color-bg-tertiary)] px-1.5 py-0.5 text-[10px] uppercase leading-none text-[var(--color-text-secondary)]"
							>
								{a.format}
							</span>
							<span class="shrink-0 text-[var(--color-text-secondary)]">
								{formatBytes(a.size_bytes)}
							</span>
							<span class="ml-auto flex shrink-0 items-center gap-1.5">
								{#if canPreview}
									<button
										type="button"
										onclick={() => (previewArtifact = a)}
										class="rounded p-1 text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-accent)]"
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
									class="rounded p-1 text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-accent)]"
									title="Download"
								>
									<svg
										xmlns="http://www.w3.org/2000/svg"
										viewBox="0 0 20 20"
										fill="currentColor"
										class="size-3.5"
									>
										<path
											d="M10.75 2.75a.75.75 0 0 0-1.5 0v8.614L6.295 8.235a.75.75 0 1 0-1.09 1.03l4.25 4.5a.75.75 0 0 0 1.09 0l4.25-4.5a.75.75 0 0 0-1.09-1.03l-2.955 3.129V2.75Z"
										/>
										<path
											d="M3.5 12.75a.75.75 0 0 0-1.5 0v2.5A2.75 2.75 0 0 0 4.75 18h10.5A2.75 2.75 0 0 0 18 15.25v-2.5a.75.75 0 0 0-1.5 0v2.5c0 .69-.56 1.25-1.25 1.25H4.75c-.69 0-1.25-.56-1.25-1.25v-2.5Z"
										/>
									</svg>
								</a>
							</span>
						</div>
					{/each}
				</div>
			</div>
		{/if}
	</div>

	{#if !report}
		<div
			class="rounded-2xl bg-[var(--color-bg-secondary)] p-8 text-sm text-[var(--color-text-secondary)]"
		>
			This plugin did not produce a readable report.
		</div>
	{:else if remainingSections.length > 0}
		{#each remainingSections as s (s.id)}
			<Section section={s} artifacts={view.artifacts} />
		{/each}
	{/if}
</div>

<ArtifactPreview artifact={previewArtifact} onclose={() => (previewArtifact = null)} />
