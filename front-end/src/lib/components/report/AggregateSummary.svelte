<script lang="ts">
	import { splitDateTime, isTerminalStatus, formatBytes } from '$lib/api/format';
	import ScoreBar from './ScoreBar.svelte';
	import PluginTile from './PluginTile.svelte';
	import Iocs from './blocks/Iocs.svelte';
	import Ttps from './blocks/Ttps.svelte';
	import type { TaskReport, Classification } from '$lib/api/types';

	interface Props {
		data: TaskReport;
	}
	let { data }: Props = $props();

	const sample = $derived(data.task.sample);

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
		return data.aggregate.indicators
			.filter((i) => HASH_KINDS.has(i.kind))
			.map((i) => ({ kind: i.kind.toUpperCase(), value: i.value }));
	});

	const running = $derived(!isTerminalStatus(data.task.status));

	const created = $derived(splitDateTime(data.task.created_on));
	const completed = $derived(
		data.task.completed_on ? splitDateTime(data.task.completed_on) : null
	);

	const durationText = $derived.by(() => {
		if (!data.task.completed_on) return null;
		const ms =
			new Date(data.task.completed_on).getTime() - new Date(data.task.created_on).getTime();
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
	});

	function scoreLabel(c: Classification | undefined): string {
		switch (c) {
			case 'clean':
				return 'Not suspicious';
			case 'suspicious':
				return 'Suspicious';
			case 'malicious':
				return 'Malicious';
			default:
				return 'Unknown';
		}
	}

	const scoreText = $derived.by(() => {
		const label = scoreLabel(data.aggregate.verdict);
		const score = data.aggregate.score;
		if (score === undefined) return label;
		return `${label}, with a score of ${score}/100`;
	});
</script>

<div class="space-y-6">
	<!-- Summary section heading -->
	<div class="flex items-baseline justify-between gap-4">
		<h1 class="text-xl font-semibold text-[var(--color-text-primary)]">Summary</h1>
		{#if running}
			<span class="rounded bg-amber-500/20 px-3 py-1 text-xs font-medium text-amber-200">
				Analysis in progress
			</span>
		{:else if data.task.status === 'failed'}
			<span class="rounded bg-red-500/20 px-3 py-1 text-xs font-medium text-red-300">
				Failed
			</span>
		{:else if data.task.status === 'cancelled'}
			<span
				class="rounded bg-[var(--color-text-secondary)]/20 px-3 py-1 text-xs font-medium text-[var(--color-text-secondary)]"
			>
				Cancelled
			</span>
		{/if}
	</div>

	<!-- Main grid: File info (wider) | Execution + Machine info (stacked, narrower) -->
	<div class="grid gap-6 md:grid-cols-3">
		<div class="space-y-4 rounded-2xl bg-[var(--color-bg-secondary)] p-8 md:col-span-2">
			<h2 class="text-sm font-medium text-[var(--color-text-secondary)]">File information</h2>
			<dl class="grid grid-cols-[max-content_1fr] items-start gap-x-6 gap-y-3 text-sm">
				{#if data.aggregate.verdict || data.aggregate.score !== undefined}
					<dt class="pt-1 text-[var(--color-text-secondary)]">Score</dt>
					<dd class="space-y-2 text-[var(--color-text-primary)]">
						<div>{scoreText}</div>
						{#if data.aggregate.score !== undefined}
							<ScoreBar
								score={data.aggregate.score}
								classification={data.aggregate.verdict}
							/>
						{/if}
					</dd>
				{/if}
				<dt class="text-[var(--color-text-secondary)]">Target</dt>
				<dd class="break-all font-mono text-xs text-[var(--color-text-primary)]">
					{data.task.target}
				</dd>
				{#if sample}
					<dt class="text-[var(--color-text-secondary)]">Size</dt>
					<dd class="text-[var(--color-text-primary)]">{formatBytes(sample.file_size)}</dd>
					<dt class="text-[var(--color-text-secondary)]">Type</dt>
					<dd class="text-[var(--color-text-primary)]">{sample.file_type}</dd>
				{/if}
				<dt class="text-[var(--color-text-secondary)]">Platform</dt>
				<dd class="text-[var(--color-text-primary)]">{data.task.platform}</dd>
				<dt class="text-[var(--color-text-secondary)]">Priority</dt>
				<dd class="text-[var(--color-text-primary)]">{data.task.priority}</dd>
				<dt class="text-[var(--color-text-secondary)]">Timeout</dt>
				<dd class="text-[var(--color-text-primary)]">{data.task.timeout}s</dd>
				{#each hashes as h (h.kind)}
					<dt class="text-[var(--color-text-secondary)]">{h.kind}</dt>
					<dd class="break-all font-mono text-xs text-[var(--color-text-primary)]">{h.value}</dd>
				{/each}
				{#if data.task.tags && data.task.tags.length > 0}
					<dt class="text-[var(--color-text-secondary)]">Tags</dt>
					<dd class="flex flex-wrap gap-1">
						{#each data.task.tags as tag (tag)}
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

		<div class="space-y-6">
			<div class="space-y-4 rounded-2xl bg-[var(--color-bg-secondary)] p-8">
				<h2 class="text-sm font-medium text-[var(--color-text-secondary)]">Execution</h2>
				<dl class="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-2 text-sm">
					<dt class="text-[var(--color-text-secondary)]">Created</dt>
					<dd class="text-[var(--color-text-primary)]">{created.date} {created.time}</dd>
					{#if completed}
						<dt class="text-[var(--color-text-secondary)]">Completed</dt>
						<dd class="text-[var(--color-text-primary)]">{completed.date} {completed.time}</dd>
					{/if}
					{#if durationText}
						<dt class="text-[var(--color-text-secondary)]">Duration</dt>
						<dd class="text-[var(--color-text-primary)]">{durationText}</dd>
					{/if}
					<dt class="text-[var(--color-text-secondary)]">Plugins</dt>
					<dd class="text-[var(--color-text-primary)]">
						{data.aggregate.plugin_count}
						<span class="ml-1 text-[var(--color-text-secondary)]">
							({data.aggregate.report_count} with reports)
						</span>
					</dd>
				</dl>
			</div>

			{#if data.task.machine_id !== null}
				<div class="space-y-4 rounded-2xl bg-[var(--color-bg-secondary)] p-8">
					<h2 class="text-sm font-medium text-[var(--color-text-secondary)]">
						Machine information
					</h2>
					<dl class="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-2 text-sm">
						<dt class="text-[var(--color-text-secondary)]">Machine</dt>
						<dd class="font-mono text-xs text-[var(--color-text-primary)]">
							{data.task.machine_id}
						</dd>
					</dl>
				</div>
			{/if}
		</div>
	</div>

	<!-- Plugins -->
	{#if data.plugins.length > 0}
		<div class="space-y-4">
			<h2
				class="text-xs font-medium uppercase tracking-wide text-[var(--color-text-secondary)]"
			>
				Plugins
			</h2>
			<div class="grid gap-4 md:grid-cols-2">
				{#each data.plugins as v (v.plugin_name)}
					<PluginTile view={v} taskId={data.task.id} />
				{/each}
			</div>
		</div>
	{:else}
		<div
			class="rounded-2xl bg-[var(--color-bg-secondary)] p-8 text-sm text-[var(--color-text-secondary)]"
		>
			{#if running}
				Analysis in progress — results will appear as plugins finish.
			{:else}
				No plugin results were produced.
			{/if}
		</div>
	{/if}

	{#if data.aggregate.indicators.length > 0}
		<div class="space-y-4 rounded-2xl bg-[var(--color-bg-secondary)] p-8">
			<h2
				class="text-xs font-medium uppercase tracking-wide text-[var(--color-text-secondary)]"
			>
				Indicators ({data.aggregate.indicators.length})
			</h2>
			<Iocs items={data.aggregate.indicators} />
		</div>
	{/if}

	{#if data.aggregate.ttps.length > 0}
		<div class="space-y-4 rounded-2xl bg-[var(--color-bg-secondary)] p-8">
			<h2
				class="text-xs font-medium uppercase tracking-wide text-[var(--color-text-secondary)]"
			>
				TTPs ({data.aggregate.ttps.length})
			</h2>
			<Ttps items={data.aggregate.ttps} />
		</div>
	{/if}
</div>
