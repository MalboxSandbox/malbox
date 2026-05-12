<script lang="ts">
	import type { TreeNode } from '$lib/api/types';
	import Self from './Tree.svelte';

	interface Props {
		nodes: TreeNode[];
		depth?: number;
	}
	let { nodes, depth = 0 }: Props = $props();

	let collapsed = $state<Set<number>>(new Set());
	let inspecting = $state<number | null>(null);

	function toggle(idx: number) {
		const next = new Set(collapsed);
		if (next.has(idx)) next.delete(idx);
		else next.add(idx);
		collapsed = next;
	}

	function parseMeta(meta: unknown): Record<string, string> {
		if (meta == null || typeof meta !== 'object') return {};
		const out: Record<string, string> = {};
		for (const [k, v] of Object.entries(meta as Record<string, unknown>)) {
			if (v == null) continue;
			out[k] = Array.isArray(v) ? v.join(', ') : typeof v === 'object' ? JSON.stringify(v) : String(v);
		}
		return out;
	}

	function extractPid(label: string): string | null {
		const m = label.match(/\(PID\s+(\d+)\)/i);
		return m ? m[1] : null;
	}

	function processName(label: string): string {
		return label
			.replace(/\s*\(PID\s+\d+\)/i, '')
			.replace(/\s*\[.*?\]/g, '')
			.trim();
	}

	function extractTags(label: string): string[] {
		const out: string[] = [];
		const m = label.match(/\[([^\]]+)\]/g);
		if (m) for (const t of m) out.push(t.slice(1, -1));
		return out;
	}

	const PROCESS_PATTERN = /\.\w{2,4}(\s|$)|\(PID\s+\d+\)/i;

	function isProcess(node: TreeNode): boolean {
		if (node.label.includes('->')) return false;
		if (PROCESS_PATTERN.test(node.label)) return true;
		if (node.children && node.children.length > 0) return true;
		return false;
	}

	function splitChildren(children: TreeNode[] | undefined): {
		processes: TreeNode[];
		activities: TreeNode[];
	} {
		if (!children || children.length === 0) return { processes: [], activities: [] };
		const processes: TreeNode[] = [];
		const activities: TreeNode[] = [];
		for (const c of children) {
			if (isProcess(c)) processes.push(c);
			else activities.push(c);
		}
		return { processes, activities };
	}

	type TagBadge = { label: string; color: string };

	function tagBadges(label: string): TagBadge[] {
		const tags = extractTags(label);
		return tags.map((t) => {
			if (/hollowed|inject/i.test(t)) return { label: t, color: 'bg-red-400' };
			if (/packed|obfuscated/i.test(t)) return { label: t, color: 'bg-amber-400' };
			return { label: t, color: 'bg-sky-400' };
		});
	}

	function activityColor(label: string): string {
		if (/credential|dump|steal|lsass/i.test(label)) return 'bg-amber-400';
		if (/beacon|c2|exfil/i.test(label)) return 'bg-purple-400';
		if (/persist|registry|run key/i.test(label)) return 'bg-rose-400';
		if (/dns|resolve/i.test(label)) return 'bg-sky-400';
		return 'bg-[var(--color-text-secondary)]';
	}
</script>

<ul class="space-y-px" class:ml-4={depth > 0} role="tree">
	{#each nodes as n, i (i)}
		{@const { processes, activities } = splitChildren(n.children)}
		{@const hasProcessChildren = processes.length > 0}
		{@const isCollapsed = collapsed.has(i)}
		{@const pid = extractPid(n.label)}
		{@const name = processName(n.label)}
		{@const meta = parseMeta(n.meta)}
		{@const tags = tagBadges(n.label)}
		{@const isInspecting = inspecting === i}
		{@const hasMeta = Object.keys(meta).length > 0}
		<li role="treeitem" aria-selected="false" class="relative">
			{#if depth > 0}
				<span class="pointer-events-none absolute -left-4 top-0 h-full w-4" aria-hidden="true">
					<span class="absolute left-1.5 top-0 h-3 w-px bg-[var(--color-border)]/25"></span>
					<span class="absolute left-1.5 top-3 h-px w-2.5 bg-[var(--color-border)]/25"></span>
					{#if i < nodes.length - 1}
						<span class="absolute left-1.5 top-3 bottom-0 w-px bg-[var(--color-border)]/25"></span>
					{/if}
				</span>
			{/if}

			<div class="flex items-center gap-1.5 rounded py-1 px-1.5 transition-colors hover:bg-[var(--color-bg-tertiary)]">
				{#if hasProcessChildren}
					<button
						type="button"
						title={isCollapsed ? 'Expand' : 'Collapse'}
						class="flex size-4 shrink-0 items-center justify-center text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]"
						onclick={() => toggle(i)}
						aria-expanded={!isCollapsed}
					>
						<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"
							class="size-3 transition-transform duration-100 {isCollapsed ? '' : 'rotate-90'}">
							<path fill-rule="evenodd" d="M7.21 14.77a.75.75 0 0 1 .02-1.06L11.168 10 7.23 6.29a.75.75 0 1 1 1.04-1.08l4.5 4.25a.75.75 0 0 1 0 1.08l-4.5 4.25a.75.75 0 0 1-1.06-.02Z" clip-rule="evenodd" />
						</svg>
					</button>
				{:else}
					<span class="size-4 shrink-0"></span>
				{/if}

				<span class="text-[13px] text-[var(--color-text-primary)]">{name}</span>

				{#if pid}
					<span class="shrink-0 font-mono text-[10px] text-[var(--color-text-secondary)]/60">{pid}</span>
				{/if}

				{#each tags as t (t.label)}
					<span class="flex shrink-0 items-center gap-1 rounded-full bg-[var(--color-bg-tertiary)] px-1.5 py-0.5 text-[10px] text-[var(--color-text-secondary)]">
						<span class="size-1.5 rounded-full {t.color}"></span>
						{t.label}
					</span>
				{/each}

				{#if meta.technique}
					<a
						href="https://attack.mitre.org/techniques/{meta.technique.split('.')[0]}/"
						target="_blank"
						rel="noopener"
						class="flex shrink-0 items-center gap-1 rounded-full bg-[var(--color-bg-tertiary)] px-1.5 py-0.5 text-[10px] text-[var(--color-accent)] transition-colors hover:bg-[var(--color-accent)]/15"
					>
						<span class="size-1.5 rounded-full bg-[var(--color-accent)]"></span>
						{meta.technique}
					</a>
				{/if}

				{#if meta.timestamp}
					<span class="ml-auto shrink-0 font-mono text-[10px] text-[var(--color-text-secondary)]/50">
						{meta.timestamp}
					</span>
				{/if}

				{#if hasMeta}
					<button
						type="button"
						title="Inspect"
						class="shrink-0 rounded p-0.5 text-[var(--color-text-secondary)]/40 transition-colors hover:text-[var(--color-text-primary)] {isInspecting ? '!text-[var(--color-text-primary)]' : ''}"
						onclick={() => (inspecting = isInspecting ? null : i)}
					>
						<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor" class="size-3">
							<path fill-rule="evenodd" d="M15 8A7 7 0 1 1 1 8a7 7 0 0 1 14 0Zm-6-3a1 1 0 1 1-2 0 1 1 0 0 1 2 0ZM6.75 8a.75.75 0 0 0 0 1.5h.75v1.75a.75.75 0 0 0 1.5 0v-2.5A.75.75 0 0 0 8.25 8h-1.5Z" clip-rule="evenodd" />
						</svg>
					</button>
				{/if}
			</div>

			{#if isInspecting}
				<div class="ml-[1.625rem] mt-0.5 mb-1 rounded-lg bg-[var(--color-bg-tertiary)] px-3 py-2">
					<dl class="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 text-[11px]">
						{#each Object.entries(meta) as [k, v] (k)}
							<dt class="text-[var(--color-text-secondary)]">{k}</dt>
							<dd class="truncate font-mono text-[var(--color-text-primary)]" title={v}>{v}</dd>
						{/each}
					</dl>
				</div>
			{/if}

			{#if activities.length > 0}
				<div class="ml-[1.625rem] flex flex-wrap gap-1.5 py-0.5">
					{#each activities as act}
						{@const actMeta = parseMeta(act.meta)}
						{@const actLabel = act.label.replace(/\s*->.*$/, '').replace(/\s*\(PID\s+\d+\)/i, '').trim()}
						{@const target = act.label.includes('->') ? act.label.split('->').pop()?.replace(/\s*\(PID\s+\d+\)/i, '').trim() : null}
						<span class="group/act relative flex items-center gap-1.5 rounded-full bg-[var(--color-bg-tertiary)] px-2 py-1 text-[11px] text-[var(--color-text-secondary)]">
							<span class="size-1.5 shrink-0 rounded-full {activityColor(act.label)}"></span>
							<span>{actLabel}</span>
							{#if target}
								<span class="text-[var(--color-text-secondary)]/50">&rarr;</span>
								<span class="font-mono text-[10px]">{target}</span>
							{/if}
							{#if actMeta.technique}
								<a
									href="https://attack.mitre.org/techniques/{actMeta.technique.split('.')[0]}/"
									target="_blank"
									rel="noopener"
									class="font-mono text-[10px] text-[var(--color-accent)] hover:underline"
								>{actMeta.technique}</a>
							{/if}
							{#if actMeta.timestamp}
								<span class="font-mono text-[10px] text-[var(--color-text-secondary)]/40">{actMeta.timestamp}</span>
							{/if}
						</span>
					{/each}
				</div>
			{/if}

			{#if hasProcessChildren && !isCollapsed}
				<Self nodes={processes} depth={depth + 1} />
			{/if}
		</li>
	{/each}
</ul>
