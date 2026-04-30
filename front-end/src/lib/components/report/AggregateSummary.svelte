<script lang="ts">
	import { splitDateTime, isTerminalStatus, formatBytes } from '$lib/api/format';
	import ScoreBar from './ScoreBar.svelte';
	import PluginTile from './PluginTile.svelte';
	import Iocs from './blocks/Iocs.svelte';
	import Ttps from './blocks/Ttps.svelte';
	import PlatformLabel from '$lib/components/ui/PlatformLabel.svelte';
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

	let copiedHash = $state<string | null>(null);

	async function copyHash(value: string) {
		await navigator.clipboard.writeText(value);
		copiedHash = value;
		setTimeout(() => {
			copiedHash = null;
		}, 1500);
	}

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

	const hasScore = $derived(data.aggregate.score != null);

	const scoreText = $derived.by(() => {
		const label = scoreLabel(data.aggregate.verdict);
		if (!hasScore) return label;
		return `${label}, with a score of ${data.aggregate.score}/100`;
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
			<dl class="grid grid-cols-[max-content_1fr] items-baseline gap-x-6 gap-y-3 text-sm">
				<dt class="text-[var(--color-text-secondary)]">Score</dt>
				<dd class="space-y-2 text-[var(--color-text-primary)]">
					{#if hasScore}
						<div>{scoreText}</div>
						<div class="max-w-48">
							<ScoreBar
								score={data.aggregate.score!}
								classification={data.aggregate.verdict}
							/>
						</div>
					{:else}
						<span class="text-[var(--color-text-secondary)]">Not available</span>
					{/if}
				</dd>
				<dt class="text-[var(--color-text-secondary)]">Target</dt>
				<dd class="break-all text-[var(--color-text-primary)]">
					{data.task.target}
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
						<span class="truncate" title={h.value}>{h.value}</span>
						<button
							type="button"
							class="relative shrink-0 transition-colors
								{copiedHash === h.value
								? 'text-[var(--color-accent)]'
								: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
							onclick={() => copyHash(h.value)}
							title="Copy to clipboard"
						>
							<svg
								xmlns="http://www.w3.org/2000/svg"
								viewBox="0 0 20 20"
								fill="currentColor"
								class="size-4 transition-all duration-200
									{copiedHash === h.value ? 'scale-0 opacity-0' : 'scale-100 opacity-100'}"
							>
								<path
									d="M7 3.5A1.5 1.5 0 0 1 8.5 2h3.879a1.5 1.5 0 0 1 1.06.44l3.122 3.12A1.5 1.5 0 0 1 17 6.622V12.5a1.5 1.5 0 0 1-1.5 1.5h-1v-3.379a3 3 0 0 0-.879-2.121L10.5 5.379A3 3 0 0 0 8.379 4.5H7v-1Z"
								/>
								<path
									d="M4.5 6A1.5 1.5 0 0 0 3 7.5v9A1.5 1.5 0 0 0 4.5 18h7a1.5 1.5 0 0 0 1.5-1.5v-5.879a1.5 1.5 0 0 0-.44-1.06L9.44 6.439A1.5 1.5 0 0 0 8.378 6H4.5Z"
								/>
							</svg>
							<svg
								xmlns="http://www.w3.org/2000/svg"
								viewBox="0 0 20 20"
								fill="currentColor"
								class="absolute inset-0 size-4 transition-all duration-200
									{copiedHash === h.value ? 'scale-100 opacity-100' : 'scale-0 opacity-0'}"
							>
								<path
									fill-rule="evenodd"
									d="M16.704 4.153a.75.75 0 0 1 .143 1.052l-8 10.5a.75.75 0 0 1-1.127.075l-4.5-4.5a.75.75 0 0 1 1.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 0 1 1.05-.143Z"
									clip-rule="evenodd"
								/>
							</svg>
						</button>
					</dd>
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
					<dt class="text-[var(--color-text-secondary)]">Platform</dt>
					<dd class="text-[var(--color-text-primary)]"><PlatformLabel platform={data.task.platform} /></dd>
					<dt class="text-[var(--color-text-secondary)]">Priority</dt>
					<dd class="text-[var(--color-text-primary)]">{data.task.priority}</dd>
					<dt class="text-[var(--color-text-secondary)]">Timeout</dt>
					<dd class="text-[var(--color-text-primary)]">{data.task.timeout}s</dd>
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
