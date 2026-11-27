<script lang="ts">
	import { SvelteSet } from 'svelte/reactivity';
	import { getTaskTree } from '#lib/api/tasks.ts';
	import { startPolling } from '#lib/api/polling.ts';
	import { formatBytes, isTerminalStatus } from '#lib/api/format.ts';
	import { isApiError } from '#lib/api/errors.ts';
	import { buildArtifactTree, type ArtifactTreeNode } from '#lib/artifacts/tree.ts';
	import { contextmenu } from '#lib/actions/contextmenu.ts';
	import VerdictBadge from './VerdictBadge.svelte';
	import ArtifactPreview from './ArtifactPreview.svelte';
	import type {
		ArtifactLink,
		ArtifactRelation,
		ArtifactState,
		JobState,
		TaskArtifact,
		TaskStatus,
		TaskTree
	} from '#lib/api/types.ts';

	interface Props {
		taskId: number;
		taskStatus: TaskStatus;
	}
	let { taskId, taskStatus }: Props = $props();

	let tree = $state<TaskTree | null>(null);
	let loadError = $state<string | null>(null);
	let previewArtifact = $state<ArtifactLink | null>(null);
	let collapsed = new SvelteSet<number>();

	const root = $derived(tree ? buildArtifactTree(tree) : null);

	const hasActiveArtifacts = $derived(
		tree?.artifacts.some((a) => a.state === 'pending' || a.state === 'running') ?? false
	);
	const shouldPoll = $derived(!isTerminalStatus(taskStatus) || hasActiveArtifacts);

	async function load(id: number) {
		try {
			const next = await getTaskTree(fetch, id);
			if (id === taskId) {
				tree = next;
				loadError = null;
			}
		} catch (err) {
			if (id === taskId) loadError = isApiError(err) ? err.message : 'Failed to load artifacts';
		}
	}

	$effect(() => {
		const id = taskId;
		// Refetch whenever the task status changes (e.g. a rescan reopened it).
		void taskStatus;
		load(id);
	});

	$effect(() => {
		if (!shouldPoll) return;
		const id = taskId;
		return startPolling(() => load(id), { intervalMs: 3000, pauseWhenHidden: true });
	});

	function toggle(id: number) {
		if (collapsed.has(id)) collapsed.delete(id);
		else collapsed.add(id);
	}

	const RELATION_ICON: Record<ArtifactRelation, string> = {
		root: '◆',
		extracted: '⇲',
		dropped: '↓',
		unpacked: '▣',
		memdump: '◫',
		downloaded: '⇣',
		pcap: '⇄',
		output: '◇'
	};

	function artifactStateClass(state: ArtifactState): string {
		switch (state) {
			case 'running':
				return 'bg-sky-500/20 text-sky-300';
			case 'pending':
				return 'bg-amber-500/20 text-amber-200';
			case 'done':
				return 'bg-emerald-500/20 text-emerald-300';
			case 'dropped':
				return 'bg-orange-500/20 text-orange-300';
			case 'canceled':
				return 'bg-[var(--color-text-secondary)]/15 text-[var(--color-text-secondary)]';
			case 'stored':
				return 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)]';
		}
	}

	function jobDotClass(state: JobState): string {
		switch (state) {
			case 'completed':
			case 'cached':
				return 'bg-emerald-400';
			case 'running':
				return 'bg-sky-400 animate-pulse';
			case 'ready':
				return 'bg-amber-400';
			case 'failed':
				return 'bg-red-400';
			case 'blocked':
			case 'skipped':
			case 'canceled':
				return 'bg-[var(--color-text-secondary)]';
		}
	}

	function previewLink(a: TaskArtifact, name: string): ArtifactLink | null {
		if (!a.content_url) return null;
		return {
			result_name: name,
			format: a.mime === 'application/json' ? 'json' : 'bytes',
			size_bytes: a.size_bytes,
			url: a.content_url,
			artifact_id: a.id,
			analyze: a.analyze
		};
	}
</script>

{#snippet branch(nodes: ArtifactTreeNode[], depth: number)}
	<ul class="space-y-px" class:ml-4={depth > 0} role={depth === 0 ? 'tree' : 'group'}>
		{#each nodes as node, i (node.artifact.id)}
			{@const a = node.artifact}
			{@const hasChildren = node.children.length > 0}
			{@const isCollapsed = collapsed.has(a.id)}
			{@const link = previewLink(a, node.name)}
			<li role="treeitem" aria-selected="false" class="relative">
				{#if depth > 0}
					<span class="pointer-events-none absolute -left-4 top-0 h-full w-4" aria-hidden="true">
						<span class="absolute left-1.5 top-0 h-3 w-px bg-[var(--color-border)]/25"></span>
						<span class="absolute left-1.5 top-3 h-px w-2.5 bg-[var(--color-border)]/25"></span>
						{#if i < nodes.length - 1}
							<span class="absolute left-1.5 top-3 bottom-0 w-px bg-[var(--color-border)]/25"
							></span>
						{/if}
					</span>
				{/if}

				<div
					class="flex flex-wrap items-center gap-1.5 rounded py-1 px-1.5 transition-colors hover:bg-[var(--color-bg-tertiary)]"
					use:contextmenu={{
						type: 'artifact',
						value: node.name,
						metadata: {
							taskId,
							artifactId: a.id,
							state: a.state,
							analyze: a.analyze,
							downloadUrl: a.content_url ?? undefined,
							onPreview: link ? () => (previewArtifact = link) : undefined
						}
					}}
				>
					{#if hasChildren}
						<button
							type="button"
							title={isCollapsed ? 'Expand' : 'Collapse'}
							class="flex size-4 shrink-0 items-center justify-center text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]"
							onclick={() => toggle(a.id)}
							aria-expanded={!isCollapsed}
						>
							<svg
								xmlns="http://www.w3.org/2000/svg"
								viewBox="0 0 20 20"
								fill="currentColor"
								class="size-3 transition-transform duration-100 {isCollapsed ? '' : 'rotate-90'}"
							>
								<path
									fill-rule="evenodd"
									d="M7.21 14.77a.75.75 0 0 1 .02-1.06L11.168 10 7.23 6.29a.75.75 0 1 1 1.04-1.08l4.5 4.25a.75.75 0 0 1 0 1.08l-4.5 4.25a.75.75 0 0 1-1.06-.02Z"
									clip-rule="evenodd"
								/>
							</svg>
						</button>
					{:else}
						<span class="size-4 shrink-0"></span>
					{/if}

					<span
						class="shrink-0 font-mono text-[11px] text-[var(--color-text-secondary)]"
						title={node.relation}>{RELATION_ICON[node.relation]}</span
					>

					{#if a.analyze}
						<a
							href="/samples/{a.sha256}"
							class="truncate text-[13px] text-[var(--color-text-primary)] hover:text-[var(--color-accent)] hover:underline"
							title={node.name}>{node.name}</a
						>
					{:else}
						<span class="truncate text-[13px] text-[var(--color-text-primary)]" title={node.name}
							>{node.name}</span
						>
					{/if}

					<span class="shrink-0 font-mono text-[10px] text-[var(--color-text-secondary)]"
						>{a.artifact_type}</span
					>
					<span class="shrink-0 text-[10px] text-[var(--color-text-secondary)]/70"
						>{formatBytes(a.size_bytes)}</span
					>
					<span
						class="shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium {artifactStateClass(
							a.state
						)}">{a.state}</span
					>

					{#if a.verdict}
						<VerdictBadge
							classification={a.verdict.classification}
							score={a.verdict.score ?? undefined}
							size="sm"
						/>
					{/if}

					{#if a.drop_reason}
						<span class="shrink-0 text-[10px] text-orange-300/80">({a.drop_reason})</span>
					{/if}

					{#if node.alsoFrom.length > 0}
						<span class="shrink-0 text-[10px] text-[var(--color-text-secondary)]/70">
							also from {node.alsoFrom.map((id) => `#${id}`).join(', ')}
						</span>
					{/if}
				</div>

				{#if node.jobs.length > 0}
					<div class="ml-[1.625rem] flex flex-wrap gap-1.5 py-0.5">
						{#each node.jobs as job (job.id)}
							<span
								class="flex items-center gap-1.5 rounded-full bg-[var(--color-bg-tertiary)] px-2 py-0.5 text-[11px] text-[var(--color-text-secondary)]"
								title={job.error_message ?? undefined}
							>
								<span class="size-1.5 shrink-0 rounded-full {jobDotClass(job.state)}"></span>
								<span>{job.analyzer}: {job.state}</span>
							</span>
						{/each}
					</div>
				{/if}

				{#if hasChildren && !isCollapsed}
					{@render branch(node.children, depth + 1)}
				{/if}
			</li>
		{/each}
	</ul>
{/snippet}

<div class="rounded-xl bg-[var(--color-bg-secondary)] p-5">
	{#if loadError}
		<div class="rounded-lg border-l-2 border-red-500 bg-red-500/10 p-3 text-sm text-red-200">
			{loadError}
		</div>
	{:else if root}
		{@render branch([root], 0)}
	{:else if tree}
		<p class="text-sm text-[var(--color-text-secondary)]">No artifacts.</p>
	{:else}
		<p class="text-sm text-[var(--color-text-secondary)]">Loading artifacts…</p>
	{/if}
</div>

<ArtifactPreview artifact={previewArtifact} onclose={() => (previewArtifact = null)} />
