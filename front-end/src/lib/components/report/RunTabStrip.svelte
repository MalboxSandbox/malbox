<script lang="ts">
	import Icon from '$lib/components/ui/Icon.svelte';
	import { icons } from '$lib/icons';
	import { BanIcon } from '@lucide/svelte';
	import type { TaskSummary } from '$lib/api/types';

	interface Props {
		tasks: TaskSummary[];
		activeRunId: string;
		latestTaskId: string | null;
		onselect: (id: string) => void;
	}

	let { tasks, activeRunId, latestTaskId, onselect }: Props = $props();
	let open = $state(false);

	const isLatest = $derived(latestTaskId !== null && activeRunId === latestTaskId);
	const isCombined = $derived(activeRunId === 'combined');
	const activeTask = $derived(tasks.find((t) => String(t.id) === activeRunId));

	function platformIcon(platform: string) {
		if (platform === 'windows') return icons.windows;
		if (platform === 'linux') return icons.linux;
		return null;
	}

	function platformName(platform: string): string {
		if (platform === 'windows') return 'Windows';
		if (platform === 'linux') return 'Linux';
		if (!platform) return 'No Platform';
		return platform.charAt(0).toUpperCase() + platform.slice(1);
	}

	function timeAgo(dateStr: string): string {
		const seconds = Math.floor((Date.now() - new Date(dateStr).getTime()) / 1000);
		if (seconds < 60) return 'just now';
		const minutes = Math.floor(seconds / 60);
		if (minutes < 60) return `${minutes}m ago`;
		const hours = Math.floor(minutes / 60);
		if (hours < 24) return `${hours}h ago`;
		const days = Math.floor(hours / 24);
		if (days < 7) return `${days}d ago`;
		const weeks = Math.floor(days / 7);
		if (weeks < 5) return `${weeks}w ago`;
		const months = Math.floor(days / 30);
		return `${months}mo ago`;
	}

	function statusBadge(status: string): { label: string; cls: string } | null {
		switch (status) {
			case 'running':
				return {
					label: 'Running',
					cls: 'bg-[var(--color-accent)]/15 text-[var(--color-accent)]'
				};
			case 'failed':
				return { label: 'Failed', cls: 'bg-red-500/15 text-red-400' };
			case 'canceled':
				return { label: 'Canceled', cls: 'bg-white/5 text-[var(--color-text-secondary)]' };
			case 'pending':
				return { label: 'Pending', cls: 'bg-yellow-500/15 text-yellow-400' };
			default:
				return null;
		}
	}
</script>

<div class="relative inline-flex items-center">
	<button
		type="button"
		class="inline-flex items-center gap-1.5 rounded-md border border-[var(--color-border)] bg-[var(--color-bg-secondary)] px-2 py-1 text-[11px] text-[var(--color-text-secondary)] transition-colors hover:border-[var(--color-text-secondary)]/40 hover:text-[var(--color-text-primary)]"
		onclick={() => (open = !open)}
	>
		{#if isLatest && activeTask}
			{@const icon = platformIcon(activeTask.platform)}
			<span class="relative flex size-2.5 shrink-0 items-center justify-center">
				<span class="absolute inset-0 rounded-full bg-emerald-400/20"></span>
				<span
					class="size-1.5 rounded-full bg-emerald-400"
					style="box-shadow: 0 0 4px oklch(0.75 0.17 155 / 0.5)"
				></span>
			</span>
			{#if icon}<Icon path={icon} class="size-3" />{/if}
			Latest
		{:else if isCombined}
			All runs
		{:else if activeTask}
			{@const icon = platformIcon(activeTask.platform)}
			<span class="relative flex size-2.5 shrink-0 items-center justify-center">
				<span class="absolute inset-0 rounded-full border border-amber-400/30"></span>
				<span class="size-1 rounded-full bg-amber-400/50"></span>
			</span>
			{#if icon}<Icon path={icon} class="size-3" />{/if}
			{platformName(activeTask.platform)}
		{:else}
			Run #{activeRunId}
		{/if}
		<svg
			class="size-2.5 transition-transform {open ? 'rotate-180' : ''}"
			viewBox="0 0 20 20"
			fill="currentColor"
		>
			<path
				fill-rule="evenodd"
				d="M5.22 8.22a.75.75 0 0 1 1.06 0L10 11.94l3.72-3.72a.75.75 0 1 1 1.06 1.06l-4.25 4.25a.75.75 0 0 1-1.06 0L5.22 9.28a.75.75 0 0 1 0-1.06Z"
				clip-rule="evenodd"
			/>
		</svg>
	</button>

	{#if open}
		<button
			type="button"
			class="fixed inset-0 z-10 cursor-default"
			tabindex="-1"
			aria-label="Close"
			onclick={() => (open = false)}
		></button>
		<div
			class="absolute right-0 top-full z-20 mt-1 min-w-[240px] rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] py-1 shadow-xl"
		>
			<div
				class="px-3 py-1.5 text-[10px] font-medium uppercase tracking-wider text-[var(--color-text-secondary)]/50"
			>
				Analysis History
			</div>
			{#each tasks as task (task.id)}
				{@const id = String(task.id)}
				{@const isTaskLatest = latestTaskId !== null && id === latestTaskId}
				{@const isActive = activeRunId === id}
				{@const icon = platformIcon(task.platform)}
				{@const badge = statusBadge(task.status)}
				<button
					type="button"
					class="flex w-full items-center gap-2 px-3 py-1.5 text-left text-[11px] transition-colors hover:bg-white/5
						{isActive ? 'text-[var(--color-text-primary)]' : 'text-[var(--color-text-secondary)]'}"
					onclick={() => {
						onselect(id);
						open = false;
					}}
				>
					{#if isTaskLatest}
						<span class="relative flex size-2.5 shrink-0 items-center justify-center">
							<span class="absolute inset-0 rounded-full bg-emerald-400/20"></span>
							<span
								class="size-1.5 rounded-full bg-emerald-400"
								style="box-shadow: 0 0 4px oklch(0.75 0.17 155 / 0.5)"
							></span>
						</span>
					{:else}
						<span class="relative flex size-2.5 shrink-0 items-center justify-center">
							<span class="absolute inset-0 rounded-full border border-amber-400/30"></span>
							<span class="size-1 rounded-full bg-amber-400/50"></span>
						</span>
					{/if}
					{#if icon}
						<Icon path={icon} class="size-3 shrink-0" />
					{:else}
						<BanIcon size={12} class="shrink-0 text-[var(--color-text-secondary)]" />
					{/if}
					<span class="flex-1">
						{platformName(task.platform)}
						<span class="text-[10px] text-[var(--color-text-secondary)]/40">#{task.id}</span>
					</span>
					{#if isTaskLatest}
						<span class="rounded bg-emerald-500/15 px-1 py-px text-[9px] text-emerald-400">
							Latest
						</span>
					{:else if task.created_on}
						<span class="text-[10px] text-[var(--color-text-secondary)]/50">
							{timeAgo(task.created_on)}
						</span>
					{/if}
					{#if badge}
						<span class="rounded px-1 py-px text-[9px] {badge.cls}">
							{badge.label}
						</span>
					{/if}
					{#if isActive}
						<svg
							class="size-3 shrink-0 text-[var(--color-accent)]"
							viewBox="0 0 20 20"
							fill="currentColor"
						>
							<path
								fill-rule="evenodd"
								d="M16.704 4.153a.75.75 0 0 1 .143 1.052l-8 10.5a.75.75 0 0 1-1.127.075l-4.5-4.5a.75.75 0 0 1 1.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 0 1 1.05-.143Z"
								clip-rule="evenodd"
							/>
						</svg>
					{/if}
				</button>
			{/each}
			<div class="my-0.5 h-px bg-[var(--color-border)]"></div>
			<button
				type="button"
				class="flex w-full items-center gap-2 px-3 py-1.5 text-left text-[11px] transition-colors hover:bg-white/5
					{isCombined ? 'text-[var(--color-text-primary)]' : 'text-[var(--color-text-secondary)]/70'}"
				onclick={() => {
					onselect('combined');
					open = false;
				}}
			>
				<span class="flex-1">All runs (combined)</span>
				{#if isCombined}
					<svg
						class="size-3 shrink-0 text-[var(--color-accent)]"
						viewBox="0 0 20 20"
						fill="currentColor"
					>
						<path
							fill-rule="evenodd"
							d="M16.704 4.153a.75.75 0 0 1 .143 1.052l-8 10.5a.75.75 0 0 1-1.127.075l-4.5-4.5a.75.75 0 0 1 1.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 0 1 1.05-.143Z"
							clip-rule="evenodd"
						/>
					</svg>
				{/if}
			</button>
		</div>
	{/if}
</div>
