<script lang="ts">
	import { page } from '$app/stores';
	import { invalidate } from '$app/navigation';
	import { startPolling } from '$lib/api/polling';
	import { formatBytes } from '$lib/api/format';
	import { rescanSample } from '$lib/api/tasks';
	import { isApiError } from '$lib/api/errors';
	import { reportStore } from '$lib/stores/report.svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import Icon from '$lib/components/ui/Icon.svelte';
	import { icons } from '$lib/icons';
	import SubmissionConfigModal from '$lib/components/SubmissionConfigModal.svelte';
	import { toasts } from '$lib/stores/toasts.svelte';
	import ScoreCard from '$lib/components/report/ScoreCard.svelte';
	import RunTabStrip from '$lib/components/report/RunTabStrip.svelte';
	import SectionDivider from '$lib/components/report/SectionDivider.svelte';
	import DataRow from '$lib/components/report/DataRow.svelte';
	import VerdictsSummary from '$lib/components/report/VerdictsSummary.svelte';
	import AIInsight from '$lib/components/report/AIInsight.svelte';
	import IOCsSection from '$lib/components/report/IOCsSection.svelte';
	import MITRESection from '$lib/components/report/MITRESection.svelte';
	import PluginDetailView from '$lib/components/report/PluginDetailView.svelte';
	import BlockRenderer from '$lib/components/report/BlockRenderer.svelte';
	import ScoreBadge from '$lib/components/report/ScoreBadge.svelte';
	import { FileTextIcon, PlayIcon, BanIcon, RefreshCwIcon, HistoryIcon } from '@lucide/svelte';
	import type {
		PluginReportView,
		Classification,
		TaskReport,
		Platform,
		KvPair,
		Block
	} from '$lib/api/types';
	import type { PageData } from './$types';

	interface Props {
		data: PageData;
	}
	let { data }: Props = $props();

	const sample = $derived(data.sample);
	const tasks = $derived(data.tasks);
	const taskReports = $derived(data.taskReports);

	const sortedTasks = $derived(
		[...tasks].sort((a, b) => new Date(b.created_on).getTime() - new Date(a.created_on).getTime())
	);

	const latestTaskId = $derived.by(() => {
		const completed = sortedTasks.filter((t) => t.status === 'completed');
		return completed.length > 0 ? String(completed[0].id) : null;
	});

	const initialRunId = $derived.by(() => {
		const fromUrl = $page.url.searchParams.get('run');
		if (fromUrl && tasks.some((t) => String(t.id) === fromUrl)) return fromUrl;
		if (latestTaskId) return latestTaskId;
		if (sortedTasks.length > 0) return String(sortedTasks[0].id);
		return 'combined';
	});
	let activeRunId = $state<string | null>(null);
	const effectiveRunId = $derived(activeRunId ?? initialRunId);
	let activePluginView = $state<PluginReportView | null>(null);

	const activeRun = $derived<TaskReport | null>(
		effectiveRunId !== 'combined'
			? (taskReports.find((r) => String(r.task.id) === effectiveRunId) ?? null)
			: null
	);

	const aggregate = $derived(activeRun ? activeRun.aggregate : data.sampleReport.aggregate);

	const isViewingHistorical = $derived(
		activeRun !== null &&
			latestTaskId !== null &&
			effectiveRunId !== latestTaskId &&
			activeRun.task.status !== 'running'
	);

	const combinedPlugins = $derived.by((): PluginReportView[] => {
		const map = new SvelteMap<string, PluginReportView>();
		const sorted = [...taskReports].sort(
			(a, b) => new Date(a.task.created_on).getTime() - new Date(b.task.created_on).getTime()
		);
		for (const report of sorted) {
			for (const p of report.plugins) {
				if (!p.failed) map.set(p.plugin_name, p);
			}
		}
		return [...map.values()];
	});

	const currentPlugins = $derived(
		activeRun ? activeRun.plugins.filter((p) => !p.failed) : combinedPlugins
	);

	const worstClassification = $derived((aggregate.verdict as Classification | null) ?? 'unknown');
	const worstScore = $derived(aggregate.score ?? 0);

	const sampleValueSet = $derived(
		new Set(
			[
				sample.md5,
				sample.sha1,
				sample.sha256,
				sample.sha512,
				sample.crc32,
				sample.ssdeep,
				sample.file_type
			].filter(Boolean)
		)
	);

	function isGenericPair(p: KvPair): boolean {
		if (sampleValueSet.has(p.value)) return true;
		const digits = p.value.replace(/\D/g, '');
		if (digits && Number(digits) === sample.file_size) return true;
		return false;
	}

	function extractOverviewFindings(blocks: Block[]): KvPair[] {
		const pairs: KvPair[] = [];
		for (const b of blocks) {
			if (b.type === 'kv') {
				for (const p of b.pairs) {
					if (!isGenericPair(p)) pairs.push(p);
				}
			}
		}
		return pairs;
	}

	const uniquePlatforms = $derived([...new Set(tasks.map((t) => t.platform))]);
	const activePlatforms = $derived(activeRun ? [activeRun.task.platform] : uniquePlatforms);
	const totalPlugins = $derived(activeRun ? activeRun.plugins.length : currentPlugins.length);
	const pluginsWithReports = $derived(
		activeRun
			? activeRun.plugins.filter((p) => p.report && !p.failed).length
			: currentPlugins.filter((p) => p.report).length
	);

	let rescanConfig = $state<{
		timeout: number | null;
		platform: Platform | null;
		tags: string[];
		plugins: string[];
		machineId: number | null;
		snapshotId: string | null;
		priority: number;
		vmMode: 'windows' | 'linux' | 'no-vm';
	}>({
		timeout: null,
		platform: null,
		tags: [],
		plugins: [],
		machineId: null,
		snapshotId: null,
		priority: 2,
		vmMode: 'windows'
	});
	let rescanning = $state(false);

	async function handleRescan() {
		if (rescanning) return;
		rescanning = true;
		try {
			const { task_id } = await rescanSample(fetch, sample.id, {
				...(rescanConfig.timeout !== null && { timeout: rescanConfig.timeout }),
				...(rescanConfig.vmMode !== 'no-vm' && { platform: rescanConfig.vmMode }),
				...(rescanConfig.tags.length > 0 && { tags: rescanConfig.tags.join(',') }),
				...(rescanConfig.plugins.length > 0 && { plugins: rescanConfig.plugins.join(',') }),
				...(rescanConfig.snapshotId !== null && { snapshot_id: rescanConfig.snapshotId }),
				priority: rescanConfig.priority
			});
			toasts.push({ kind: 'success', message: `Re-analysis task #${task_id} submitted.` });
			await invalidate('malbox:sample');
			await invalidate('malbox:sample-report');
			activeRunId = String(task_id);
		} catch (err) {
			const msg = isApiError(err) ? err.message : 'Re-analysis failed.';
			toasts.push({ kind: 'error', message: msg });
		} finally {
			rescanning = false;
		}
	}

	$effect(() => {
		reportStore.setSamplePage({
			sha256: sample.sha256,
			sample,
			tasks,
			taskReports,
			activeRunId: effectiveRunId
		});
		return () => reportStore.setSamplePage(null);
	});

	const hasPendingRuns = $derived(
		tasks.some((t) => !['completed', 'failed', 'canceled'].includes(t.status))
	);

	$effect(() => {
		if (!hasPendingRuns) return;
		const stop = startPolling(
			async () => {
				await invalidate('malbox:sample-report');
			},
			{ intervalMs: 3000, pauseWhenHidden: true }
		);
		return stop;
	});
</script>

{#if activePluginView}
	<PluginDetailView
		view={activePluginView}
		sample={{
			file_size: sample.file_size,
			file_type: sample.file_type,
			md5: sample.md5,
			crc32: sample.crc32,
			sha1: sample.sha1,
			sha256: sample.sha256,
			sha512: sample.sha512,
			ssdeep: sample.ssdeep
		}}
		onback={() => (activePluginView = null)}
	/>
{:else}
	<div class="space-y-0">
		<!-- Breadcrumb -->
		<div class="mb-3.5 flex items-center gap-1.5 text-xs text-[var(--color-text-secondary)]">
			<a href="/submissions" class="hover:text-[var(--color-text-primary)] transition-colors"
				>Submissions</a
			>
			<svg class="size-3 text-[var(--color-border)]" viewBox="0 0 20 20" fill="currentColor">
				<path
					fill-rule="evenodd"
					d="M7.21 14.77a.75.75 0 0 1 .02-1.06L11.168 10 7.23 6.29a.75.75 0 1 1 1.04-1.08l4.5 4.25a.75.75 0 0 1 0 1.08l-4.5 4.25a.75.75 0 0 1-1.06-.02Z"
					clip-rule="evenodd"
				/>
			</svg>
			<span class="text-[var(--color-text-primary)]"
				>{data.sampleReport.sample.sha256.slice(0, 12)}...</span
			>
		</div>

		{#if isViewingHistorical && activeRun}
			<div
				class="mb-3.5 flex items-center gap-2 rounded-lg border border-amber-500/20 bg-amber-500/5 px-3.5 py-2"
			>
				<HistoryIcon size={14} class="shrink-0 text-amber-400" />
				<span class="text-xs text-amber-200/80">
					Viewing historical analysis from {new Date(
						activeRun.task.completed_on ?? activeRun.task.created_on
					).toLocaleDateString()}
				</span>
				<button
					class="ml-auto text-xs font-medium text-[var(--color-accent)] transition-colors hover:text-[var(--color-accent)]/80"
					onclick={() => (activeRunId = null)}
				>
					View latest
				</button>
			</div>
		{/if}

		<!-- Overview section -->
		<div id="section-overview" class="mb-1.5">
			<!-- Sample header -->
			<div class="mb-3.5">
				<div class="flex items-center gap-2.5">
					<h1 class="text-lg font-semibold text-[var(--color-text-primary)]">
						{sample.sha256.slice(0, 12)}...{sample.sha256.slice(-6)}
					</h1>
					<span class="ml-auto flex items-center gap-2">
						<RunTabStrip
							tasks={sortedTasks}
							activeRunId={effectiveRunId}
							{latestTaskId}
							onselect={(id) => (activeRunId = id)}
						/>
						{#snippet rescanIcon()}
							<RefreshCwIcon size={12} class={rescanning ? 'animate-spin' : ''} />
						{/snippet}
						<SubmissionConfigModal
							config={rescanConfig}
							onApply={(c) => {
								rescanConfig = c;
								handleRescan();
							}}
							triggerLabel={rescanning ? 'Submitting...' : 'Re-analyze'}
							triggerClass="inline-flex items-center gap-1.5 rounded-md border border-[var(--color-border)] bg-[var(--color-bg-secondary)] px-2.5 py-1 text-[11px] font-medium text-[var(--color-text-secondary)] transition-colors hover:border-[var(--color-text-secondary)]/40 hover:text-[var(--color-text-primary)]"
							triggerIcon={rescanIcon}
						/>
					</span>
				</div>
			</div>

			<!-- File identity + Analysis summary / Score -->
			<div class="grid grid-cols-2 gap-3">
				<div class="flex flex-col rounded-xl bg-[var(--color-bg-secondary)] p-5">
					<span
						class="flex items-center gap-2 text-sm font-medium text-[var(--color-text-primary)]"
					>
						<FileTextIcon size={15} class="text-[var(--color-text-secondary)]" />
						File Information
					</span>
					<div class="mt-4 flex flex-1 flex-col justify-between">
						<DataRow
							label="Type"
							value={sample.file_type}
							context={{ type: 'generic', metadata: { label: 'Type' } }}
						/>
						<DataRow
							label="Size"
							value={formatBytes(sample.file_size)}
							context={{ type: 'generic', metadata: { label: 'Size' } }}
						/>
						<DataRow
							label="MD5"
							value={sample.md5}
							copyable
							context={{ type: 'hash', subtype: 'md5' }}
						/>
						<DataRow
							label="SHA1"
							value={sample.sha1}
							copyable
							context={{ type: 'hash', subtype: 'sha1' }}
						/>
						<DataRow
							label="SHA256"
							value={sample.sha256}
							copyable
							context={{ type: 'hash', subtype: 'sha256' }}
						/>
						<DataRow
							label="SHA512"
							value={sample.sha512}
							copyable
							context={{ type: 'hash', subtype: 'sha512' }}
						/>
						<DataRow
							label="CRC32"
							value={sample.crc32}
							copyable
							context={{ type: 'hash', subtype: 'crc32' }}
						/>
						<DataRow
							label="SSDEEP"
							value={sample.ssdeep}
							copyable
							context={{ type: 'hash', subtype: 'ssdeep' }}
						/>
					</div>
				</div>
				<div class="flex flex-col gap-3">
					<ScoreCard score={worstScore} classification={worstClassification} />
					<div class="flex flex-1 flex-col rounded-xl bg-[var(--color-bg-secondary)] p-5">
						<span
							class="flex items-center gap-2 text-sm font-medium text-[var(--color-text-primary)]"
						>
							<PlayIcon size={15} class="text-[var(--color-text-secondary)]" />
							Execution Information
						</span>
						<div class="mt-4 flex flex-1 flex-col justify-between">
							{#each activePlatforms as platform (platform)}
								<DataRow label="Platform">
									{#if platform === 'windows'}
										<Icon path={icons.windows} class="size-4 text-[var(--color-text-secondary)]" />
										<span class="text-sm text-[var(--color-text-primary)]">Windows</span>
									{:else if platform === 'linux'}
										<Icon path={icons.linux} class="size-4 text-[var(--color-text-secondary)]" />
										<span class="text-sm text-[var(--color-text-primary)]">Linux</span>
									{:else}
										<BanIcon size={16} class="text-[var(--color-text-secondary)]" />
										<span class="text-sm text-[var(--color-text-primary)]">No Platform</span>
									{/if}
								</DataRow>
							{/each}
							<DataRow label="Plugins">
								<span class="text-sm text-[var(--color-text-primary)]">
									{totalPlugins}
									<span class="text-[var(--color-text-secondary)]">
										({pluginsWithReports} with reports)
									</span>
								</span>
							</DataRow>
							{#if activeRun}
								<DataRow label="Priority" value={String(activeRun.task.priority)} />
								<DataRow label="Timeout" value="{activeRun.task.timeout}s" />
								<DataRow label="Status" value={activeRun.task.status} />
								{#if activeRun.task.started_on && activeRun.task.completed_on}
									<DataRow
										label="Duration"
										value="{Math.round(
											(new Date(activeRun.task.completed_on).getTime() -
												new Date(activeRun.task.started_on).getTime()) /
												1000
										)}s"
									/>
								{/if}
							{/if}
							<DataRow
								label="Submitted"
								value={tasks[0]?.created_on ? new Date(tasks[0].created_on).toLocaleString() : '—'}
							/>
						</div>
					</div>
				</div>
			</div>
		</div>

		<!-- Verdicts section -->
		<SectionDivider
			id="verdicts"
			title="Verdicts"
			count={`${currentPlugins.length} engine${currentPlugins.length === 1 ? '' : 's'}`}
		/>
		<div class="flex flex-col gap-3">
			<AIInsight plugins={currentPlugins} />
			<VerdictsSummary plugins={currentPlugins} onpluginclick={(p) => (activePluginView = p)} />
		</div>

		<!-- Per-plugin report sections -->
		{#each currentPlugins as plugin (plugin.plugin_name)}
			{@const pluginReport = plugin.report}
			{@const displayName =
				pluginReport?.plugin.display_name ?? pluginReport?.plugin.id ?? plugin.plugin_name}
			{@const allSections = pluginReport?.sections ?? []}
			{@const overview = allSections.find((s) => s.title.toLowerCase() === 'overview')}
			{@const overviewBlocks = overview?.blocks ?? []}
			{@const callouts = overviewBlocks.filter((b) => b.type === 'callout')}
			{@const findings = extractOverviewFindings(overviewBlocks)}
			{@const contentSections = allSections.filter((s) => s !== overview)}
			{@const hasContent = contentSections.length > 0 || callouts.length > 0 || findings.length > 0}
			{#if hasContent}
				<SectionDivider id="plugin-{plugin.plugin_name}" title={displayName}>
					{#if pluginReport?.verdict}
						<ScoreBadge
							score={pluginReport.verdict.score ?? 0}
							classification={pluginReport.verdict.classification}
						/>
					{/if}
				</SectionDivider>
				<div class="flex flex-col gap-3">
					{#each callouts as block, i (i)}
						<BlockRenderer {block} artifacts={plugin.artifacts} />
					{/each}

					{#if findings.length > 0}
						<div class="flex flex-wrap gap-1.5">
							{#each findings as p (p.key)}
								<span
									class="rounded bg-[var(--color-bg-tertiary)] px-2 py-0.5 text-xs text-[var(--color-text-secondary)]"
								>
									{p.key}:
									<span class="text-[var(--color-text-primary)]">{p.value}</span>
								</span>
							{/each}
						</div>
					{/if}

					{#each contentSections as section (section.id)}
						<div
							id="section-{plugin.plugin_name}-{section.id}"
							class="rounded-xl bg-[var(--color-bg-secondary)] p-5"
						>
							<span class="text-sm font-medium text-[var(--color-text-primary)]">
								{section.title}
							</span>
							<div class="mt-3 space-y-3">
								{#each section.blocks ?? [] as block, i (i)}
									<BlockRenderer {block} artifacts={plugin.artifacts} />
								{/each}
							</div>
						</div>
					{/each}
				</div>
			{/if}
		{/each}

		<!-- Indicators of Compromise -->
		{#if aggregate.indicators.length > 0}
			<SectionDivider
				id="indicators"
				title="Indicators of Compromise"
				count={String(aggregate.indicators.length)}
			/>
			<IOCsSection indicators={aggregate.indicators} />
		{/if}

		<!-- MITRE ATT&CK -->
		{#if aggregate.ttps.length > 0}
			<SectionDivider
				id="mitre"
				title="MITRE ATT&CK"
				count={`${aggregate.ttps.length} technique${aggregate.ttps.length === 1 ? '' : 's'}`}
			/>
			<MITRESection ttps={aggregate.ttps} />
		{/if}

		<div class="h-12"></div>
	</div>
{/if}
