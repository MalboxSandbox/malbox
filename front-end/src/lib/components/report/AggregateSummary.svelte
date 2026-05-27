<script lang="ts">
	import { splitDateTime, isTerminalStatus, formatBytes } from '$lib/api/format';
	import { cancelTask } from '$lib/api/tasks';
	import ScoreCard from './ScoreCard.svelte';
	import PluginTile from './PluginTile.svelte';
	import ThreatOverview from './ThreatOverview.svelte';
	import CopyButton from '$lib/components/ui/CopyButton.svelte';
	import PlatformLabel from '$lib/components/ui/PlatformLabel.svelte';
	import { FileTextIcon, PlayIcon } from '@lucide/svelte';
	import type { Task, TaskReport } from '$lib/api/types';
	import { contextmenu } from '$lib/actions/contextmenu';

	interface Props {
		task: Task;
		report: TaskReport | null;
	}
	let { task, report }: Props = $props();

	const sample = $derived(task.sample);

	// Hashes from the sample when present (file uploads); otherwise fall back
	// to any hash-kind indicators a plugin emitted. Avoids double-rendering
	// when both are available.
	const HASH_KINDS = new Set(['md5', 'sha1', 'sha256', 'sha512', 'ssdeep', 'crc32']);
	type HashPair = { kind: string; value: string };
	const hashes = $derived.by<HashPair[]>(() => {
		if (sample) {
			return [
				{ kind: 'MD5', value: sample.md5 },
				{ kind: 'SHA1', value: sample.sha1 },
				{ kind: 'SHA256', value: sample.sha256 },
				{ kind: 'SHA512', value: sample.sha512 },
				{ kind: 'CRC32', value: sample.crc32 },
				{ kind: 'SSDEEP', value: sample.ssdeep }
			];
		}
		if (!report) return [];
		return report.aggregate.indicators
			.filter((i) => HASH_KINDS.has(i.kind))
			.map((i) => ({ kind: i.kind.toUpperCase(), value: i.value }));
	});

	const running = $derived(!isTerminalStatus(task.status));

	const created = $derived(splitDateTime(task.created_on));
	const started = $derived(task.started_on ? splitDateTime(task.started_on) : null);
	const completed = $derived(task.completed_on ? splitDateTime(task.completed_on) : null);

	function formatDuration(ms: number): string | null {
		if (!Number.isFinite(ms) || ms <= 0) return null;
		const sec = Math.floor(ms / 1000);
		const days = Math.floor(sec / 86400);
		const hours = Math.floor((sec % 86400) / 3600);
		const mins = Math.floor((sec % 3600) / 60);
		const secs = sec % 60;
		if (days > 0) return `${days}d ${hours}h`;
		if (hours > 0) return `${hours}h ${mins}m`;
		if (mins > 0) return `${mins}m ${secs}s`;
		return `${secs}s`;
	}

	const analysisDuration = $derived.by(() => {
		if (!task.completed_on || !task.started_on) return null;
		const ms = new Date(task.completed_on).getTime() - new Date(task.started_on).getTime();
		return formatDuration(ms);
	});

	const queueDuration = $derived.by(() => {
		if (!task.started_on) return null;
		const ms = new Date(task.started_on).getTime() - new Date(task.created_on).getTime();
		return formatDuration(ms);
	});

	const hasScore = $derived(report?.aggregate.score != null);

	const successfulPlugins = $derived(report?.plugins.filter((p) => !p.failed) ?? []);
	const failedPlugins = $derived(report?.plugins.filter((p) => p.failed) ?? []);
	const failedNames = $derived(
		failedPlugins.map((p) => p.report?.plugin.display_name ?? p.report?.plugin.id ?? p.plugin_name)
	);

	let canceling = $state(false);

	async function handleCancel() {
		if (canceling) return;
		canceling = true;
		try {
			await cancelTask(fetch, task.id);
		} catch (e) {
			console.error('Failed to cancel task:', e);
			canceling = false;
		}
	}
</script>

<div class="space-y-6">
	<!-- Summary section heading -->
	<div class="flex items-baseline justify-between gap-4">
		<h1 class="text-xl font-semibold text-[var(--color-text-primary)]">Summary</h1>
		{#if running}
			<div class="flex items-center gap-2">
				<span class="rounded bg-amber-500/20 px-3 py-1 text-xs font-medium text-amber-200">
					Analysis in progress
				</span>
				<button
					type="button"
					disabled={canceling}
					onclick={handleCancel}
					class="flex items-center gap-1 rounded bg-[var(--color-text-secondary)]/15 px-3 py-1 text-xs font-medium text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-text-secondary)]/25 disabled:cursor-not-allowed disabled:opacity-50"
				>
					<svg
						xmlns="http://www.w3.org/2000/svg"
						viewBox="0 0 20 20"
						fill="currentColor"
						class="size-3.5"
					>
						<path
							fill-rule="evenodd"
							d="M10 18a8 8 0 1 0 0-16 8 8 0 0 0 0 16ZM8.28 7.22a.75.75 0 0 0-1.06 1.06L8.94 10l-1.72 1.72a.75.75 0 1 0 1.06 1.06L10 11.06l1.72 1.72a.75.75 0 1 0 1.06-1.06L11.06 10l1.72-1.72a.75.75 0 0 0-1.06-1.06L10 8.94 8.28 7.22Z"
							clip-rule="evenodd"
						/>
					</svg>
					{canceling ? 'Cancelling...' : 'Cancel'}
				</button>
			</div>
		{:else if task.status === 'failed'}
			<span class="rounded bg-red-500/20 px-3 py-1 text-xs font-medium text-red-300"> Failed </span>
		{:else if task.status === 'canceled'}
			<span
				class="rounded bg-[var(--color-text-secondary)]/20 px-3 py-1 text-xs font-medium text-[var(--color-text-secondary)]"
			>
				Canceled
			</span>
		{/if}
	</div>

	<!-- Main grid: File info (wider) | Score + Execution + Machine info (stacked, narrower) -->
	<div class="grid gap-6 md:grid-cols-3">
		<div class="space-y-4 rounded-2xl bg-[var(--color-bg-secondary)] p-8 md:col-span-2">
			<h2 class="flex items-center gap-2 text-sm font-medium text-[var(--color-text-secondary)]">
				<FileTextIcon class="size-4" />
				File information
			</h2>
			<dl class="grid grid-cols-[max-content_1fr] items-baseline gap-x-6 gap-y-3 text-sm">
				<dt class="text-[var(--color-text-secondary)]">Target</dt>
				<dd class="break-all text-[var(--color-text-primary)]">
					{task.target}
				</dd>
				{#if sample}
					<dt class="text-[var(--color-text-secondary)]">Size</dt>
					<dd class="text-[var(--color-text-primary)]">{formatBytes(sample.file_size)}</dd>
					<dt class="text-[var(--color-text-secondary)]">Type</dt>
					<dd class="text-[var(--color-text-primary)]">{sample.file_type}</dd>
				{/if}
				{#each hashes as h (h.kind)}
					<dt class="text-[var(--color-text-secondary)]">{h.kind}</dt>
					<dd class="flex min-w-0 items-center gap-2 text-[var(--color-text-primary)]">
						<span
							class="truncate"
							title={h.value}
							use:contextmenu={{ type: 'hash', value: h.value, subtype: h.kind.toLowerCase() }}
							>{h.value}</span
						>
						<CopyButton value={h.value} />
					</dd>
				{/each}
				{#if task.tags && task.tags.length > 0}
					<dt class="text-[var(--color-text-secondary)]">Tags</dt>
					<dd class="flex flex-wrap gap-1">
						{#each task.tags as tag (tag)}
							<span
								class="rounded bg-[var(--color-bg-tertiary)] px-2 py-0.5 text-xs text-[var(--color-text-secondary)]"
							>
								{tag}
							</span>
						{/each}
					</dd>
				{/if}
			</dl>
		</div>

		<div class="flex flex-col gap-6">
			{#if hasScore && report}
				<ScoreCard score={report.aggregate.score!} classification={report.aggregate.verdict} />
			{:else if !report}
				<div class="h-32 animate-pulse rounded-2xl bg-[var(--color-bg-secondary)]"></div>
			{/if}

			<div class="flex flex-1 flex-col gap-4 rounded-2xl bg-[var(--color-bg-secondary)] p-8">
				<h2 class="flex items-center gap-2 text-sm font-medium text-[var(--color-text-secondary)]">
					<PlayIcon class="size-4" />
					Execution
				</h2>
				<dl class="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-2 text-sm">
					<dt class="text-[var(--color-text-secondary)]">Platform</dt>
					<dd class="text-[var(--color-text-primary)]">
						<PlatformLabel platform={task.platform} />
					</dd>
					<dt class="text-[var(--color-text-secondary)]">Priority</dt>
					<dd class="text-[var(--color-text-primary)]">{task.priority}</dd>
					<dt class="text-[var(--color-text-secondary)]">Timeout</dt>
					<dd class="text-[var(--color-text-primary)]">{task.timeout}s</dd>
					<dt class="text-[var(--color-text-secondary)]">Created</dt>
					<dd class="text-[var(--color-text-primary)]">{created.date} {created.time}</dd>
					{#if started}
						<dt class="text-[var(--color-text-secondary)]">Started</dt>
						<dd class="text-[var(--color-text-primary)]">
							{started.date}
							{started.time}
							{#if queueDuration}
								<span class="ml-1 text-xs text-[var(--color-text-secondary)]"
									>(queued {queueDuration})</span
								>
							{/if}
						</dd>
					{/if}
					{#if completed}
						<dt class="text-[var(--color-text-secondary)]">Completed</dt>
						<dd class="text-[var(--color-text-primary)]">{completed.date} {completed.time}</dd>
					{/if}
					{#if analysisDuration}
						<dt class="text-[var(--color-text-secondary)]">Duration</dt>
						<dd class="text-[var(--color-text-primary)]">{analysisDuration}</dd>
					{/if}
					{#if report}
						<dt class="text-[var(--color-text-secondary)]">Plugins</dt>
						<dd class="text-[var(--color-text-primary)]">
							{report.aggregate.plugin_count}
							<span class="ml-1 text-[var(--color-text-secondary)]">
								({report.aggregate.report_count} with reports)
							</span>
						</dd>
					{/if}
				</dl>
			</div>

			{#if task.machine_id !== null}
				<div class="space-y-4 rounded-2xl bg-[var(--color-bg-secondary)] p-8">
					<h2 class="text-sm font-medium text-[var(--color-text-secondary)]">
						Machine information
					</h2>
					<dl class="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-2 text-sm">
						<dt class="text-[var(--color-text-secondary)]">Machine</dt>
						<dd class="font-mono text-xs text-[var(--color-text-primary)]">
							{task.machine_id}
						</dd>
					</dl>
				</div>
			{/if}
		</div>
	</div>

	<!-- Plugins -->
	{#if report}
		{#if report.plugins.length > 0}
			<div class="space-y-4">
				<div class="flex items-center gap-3">
					<h2
						class="text-xs font-medium uppercase tracking-wide text-[var(--color-text-secondary)]"
					>
						Plugins
					</h2>
					{#if failedPlugins.length > 0}
						<span
							class="group/failed relative inline-flex items-center gap-1.5 rounded-full bg-red-500/10 px-2.5 py-1 text-[11px] font-medium text-red-400"
							title={failedNames.join(', ')}
						>
							<svg
								xmlns="http://www.w3.org/2000/svg"
								viewBox="0 0 16 16"
								fill="currentColor"
								class="size-3"
							>
								<path
									fill-rule="evenodd"
									d="M6.701 2.25c.577-1 2.02-1 2.598 0l5.196 9a1.5 1.5 0 0 1-1.299 2.25H2.804a1.5 1.5 0 0 1-1.3-2.25l5.197-9ZM8 4a.75.75 0 0 1 .75.75v3a.75.75 0 0 1-1.5 0v-3A.75.75 0 0 1 8 4Zm0 8a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z"
									clip-rule="evenodd"
								/>
							</svg>
							{failedPlugins.length} failed
							<div
								class="pointer-events-none absolute left-0 top-full z-10 mt-2 w-max max-w-xs rounded-lg bg-[var(--color-bg-secondary)] p-3 opacity-0 shadow-lg ring-1 ring-[var(--color-border)] transition-opacity group-hover/failed:pointer-events-auto group-hover/failed:opacity-100"
							>
								<p class="mb-1.5 text-[11px] font-medium text-[var(--color-text-secondary)]">
									Failed plugins
								</p>
								<ul class="space-y-1">
									{#each failedNames as name (name)}
										<li class="text-xs text-red-300">{name}</li>
									{/each}
								</ul>
							</div>
						</span>
					{/if}
				</div>
				{#if successfulPlugins.length > 0}
					<div class="grid gap-4 md:grid-cols-2">
						{#each successfulPlugins as v (v.plugin_name)}
							<PluginTile view={v} taskId={task.id} />
						{/each}
					</div>
				{/if}
			</div>
		{:else}
			<div
				class="rounded-2xl bg-[var(--color-bg-secondary)] p-8 text-sm text-[var(--color-text-secondary)]"
			>
				{#if running}
					Analysis in progress - results will appear as plugins finish.
				{:else}
					No plugin results were produced.
				{/if}
			</div>
		{/if}

		{#if report.aggregate.indicators.length > 0 || report.aggregate.ttps.length > 0}
			<ThreatOverview indicators={report.aggregate.indicators} ttps={report.aggregate.ttps} />
		{/if}
	{:else}
		<!-- Loading skeleton for report-dependent sections -->
		<div class="animate-pulse space-y-4">
			<div class="h-4 w-24 rounded bg-[var(--color-bg-card)]"></div>
			<div class="grid gap-4 md:grid-cols-2">
				<div class="h-32 rounded-2xl bg-[var(--color-bg-secondary)]"></div>
				<div class="h-32 rounded-2xl bg-[var(--color-bg-secondary)]"></div>
			</div>
		</div>
	{/if}
</div>
