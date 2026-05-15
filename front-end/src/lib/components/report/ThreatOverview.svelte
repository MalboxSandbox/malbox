<script lang="ts">
	import { SvelteSet } from 'svelte/reactivity';
	import { slide } from 'svelte/transition';
	import CopyButton from '$lib/components/ui/CopyButton.svelte';
	import type { Indicator, Ttp } from '$lib/api/types';

	interface Props {
		indicators: Indicator[];
		ttps: Ttp[];
	}
	let { indicators, ttps }: Props = $props();

	const CATEGORY_DEFS = [
		{
			id: 'network',
			label: 'Network',
			kinds: new Set(['domain', 'ip', 'ipv4', 'ipv6', 'url', 'email', 'uri', 'hostname']),
			dot: 'bg-sky-400'
		},
		{
			id: 'hash',
			label: 'Hashes',
			kinds: new Set(['md5', 'sha1', 'sha256', 'sha512', 'ssdeep', 'crc32']),
			dot: 'bg-amber-400'
		},
		{
			id: 'filesystem',
			label: 'Filesystem',
			kinds: new Set(['filename', 'filepath', 'path', 'file']),
			dot: 'bg-violet-400'
		},
		{
			id: 'system',
			label: 'System',
			kinds: new Set(['mutex', 'registry', 'process', 'service', 'pipe', 'command', 'cmd']),
			dot: 'bg-rose-400'
		}
	] as const;

	const OTHER_STYLE = { dot: 'bg-slate-400' };

	interface CategoryGroup {
		id: string;
		label: string;
		dot: string;
		kinds: Map<string, Indicator[]>;
		total: number;
	}

	const categories = $derived.by(() => {
		// eslint-disable-next-line svelte/prefer-svelte-reactivity -- local computation variable
		const groups = new Map<string, CategoryGroup>();
		for (const def of CATEGORY_DEFS) {
			groups.set(def.id, {
				id: def.id,
				label: def.label,
				dot: def.dot,
				kinds: new Map(),
				total: 0
			});
		}
		groups.set('other', {
			id: 'other',
			label: 'Other',
			...OTHER_STYLE,
			kinds: new Map(),
			total: 0
		});

		for (const ind of indicators) {
			const kindLower = ind.kind.toLowerCase();
			let catId = 'other';
			for (const def of CATEGORY_DEFS) {
				if (def.kinds.has(kindLower)) {
					catId = def.id;
					break;
				}
			}
			const group = groups.get(catId)!;
			if (!group.kinds.has(ind.kind)) group.kinds.set(ind.kind, []);
			group.kinds.get(ind.kind)!.push(ind);
			group.total++;
		}

		return Array.from(groups.values()).filter((g) => g.total > 0);
	});

	let expanded = new SvelteSet<string>();

	function toggleCategory(id: string) {
		if (expanded.has(id)) expanded.delete(id);
		else expanded.add(id);
	}

	function attackUrl(id: string): string {
		const parent = id.split('.')[0];
		return `https://attack.mitre.org/techniques/${parent}/`;
	}

	const hasIndicators = $derived(indicators.length > 0);
	const hasTtps = $derived(ttps.length > 0);
</script>

<div class="space-y-6">
	{#if hasIndicators}
		<div class="space-y-4">
			<h2 class="text-xs font-medium uppercase tracking-wide text-[var(--color-text-secondary)]">
				Indicators
			</h2>

			<div class="space-y-2">
				{#each categories as cat (cat.id)}
					<div class="overflow-hidden rounded-2xl bg-[var(--color-bg-secondary)]">
						<button
							type="button"
							aria-expanded={expanded.has(cat.id)}
							onclick={() => toggleCategory(cat.id)}
							class="group flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-[var(--color-bg-card)]"
						>
							<span class="size-1.5 shrink-0 rounded-full {cat.dot}"></span>
							<span class="text-sm font-medium text-[var(--color-text-primary)]">
								{cat.label}
							</span>
							<span class="text-xs tabular-nums text-[var(--color-text-secondary)]">
								{cat.total}
							</span>

							<div class="ml-1 flex flex-1 flex-wrap gap-1.5">
								{#each Array.from(cat.kinds.entries()) as [kind, list] (kind)}
									<span
										class="rounded bg-[var(--color-bg-card)] px-1.5 py-0.5 text-[11px] text-[var(--color-text-secondary)]"
									>
										{kind}
										<span class="ml-0.5 opacity-60">{list.length}</span>
									</span>
								{/each}
							</div>

							<svg
								xmlns="http://www.w3.org/2000/svg"
								viewBox="0 0 20 20"
								fill="currentColor"
								class="size-4 shrink-0 text-[var(--color-text-secondary)] transition-transform duration-200"
								class:rotate-180={expanded.has(cat.id)}
							>
								<path
									fill-rule="evenodd"
									d="M5.22 8.22a.75.75 0 0 1 1.06 0L10 11.94l3.72-3.72a.75.75 0 1 1 1.06 1.06l-4.25 4.25a.75.75 0 0 1-1.06 0L5.22 9.28a.75.75 0 0 1 0-1.06Z"
									clip-rule="evenodd"
								/>
							</svg>
						</button>

						{#if expanded.has(cat.id)}
							<div transition:slide={{ duration: 200 }}>
								{#each Array.from(cat.kinds.entries()).sort( (a, b) => a[0].localeCompare(b[0]) ) as [kind, list] (kind)}
									<div class="border-t border-[var(--color-border)]/40 px-4 py-3">
										<div
											class="mb-2 text-[11px] font-medium uppercase tracking-wide text-[var(--color-text-secondary)]"
										>
											{kind}
										</div>
										{#each list as ind, i (kind + ':' + i)}
											<div
												class="flex items-center gap-2 py-1.5 {i > 0
													? 'border-t border-[var(--color-border)]/20'
													: ''}"
											>
												<span class="min-w-0 break-all text-sm text-[var(--color-text-primary)]">
													{ind.value}
												</span>
												<CopyButton value={ind.value} size="sm" />
												{#if ind.context}
													<span class="ml-auto shrink-0 text-xs text-[var(--color-text-secondary)]">
														{ind.context}
													</span>
												{/if}
											</div>
										{/each}
									</div>
								{/each}
							</div>
						{/if}
					</div>
				{/each}
			</div>
		</div>
	{/if}

	{#if hasTtps}
		<div class="space-y-4">
			<h2 class="text-xs font-medium uppercase tracking-wide text-[var(--color-text-secondary)]">
				MITRE ATT&CK
			</h2>

			<div class="grid gap-4 sm:grid-cols-2">
				{#each ttps as t (t.id)}
					<a
						href={attackUrl(t.id)}
						target="_blank"
						rel="noopener"
						class="group flex items-start gap-3 rounded-2xl bg-[var(--color-bg-secondary)] p-5 transition-colors hover:bg-[var(--color-bg-tertiary)]"
					>
						<span
							class="shrink-0 rounded-md bg-[var(--color-bg-card)] px-2 py-0.5 font-mono text-xs text-[var(--color-accent)] transition-colors group-hover:bg-[var(--color-accent)]/10"
						>
							{t.id}
						</span>
						<div class="min-w-0 flex-1">
							<div class="text-sm text-[var(--color-text-primary)]">{t.name}</div>
							{#if t.evidence}
								<div class="mt-0.5 line-clamp-2 text-xs text-[var(--color-text-secondary)]">
									{t.evidence}
								</div>
							{/if}
						</div>
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 16 16"
							fill="currentColor"
							class="mt-1 size-3 shrink-0 text-[var(--color-text-secondary)] opacity-0 transition-opacity group-hover:opacity-100"
						>
							<path
								d="M6.22 8.72a.75.75 0 0 0 1.06 1.06l5.22-5.22v1.69a.75.75 0 0 0 1.5 0v-3.5a.75.75 0 0 0-.75-.75h-3.5a.75.75 0 0 0 0 1.5h1.69L6.22 8.72Z"
							/>
							<path
								d="M3.5 6.75c0-.69.56-1.25 1.25-1.25H7A.75.75 0 0 0 7 4H4.75A2.75 2.75 0 0 0 2 6.75v4.5A2.75 2.75 0 0 0 4.75 14h4.5A2.75 2.75 0 0 0 12 11.25V9a.75.75 0 0 0-1.5 0v2.25c0 .69-.56 1.25-1.25 1.25h-4.5c-.69 0-1.25-.56-1.25-1.25v-4.5Z"
							/>
						</svg>
					</a>
				{/each}
			</div>
		</div>
	{/if}
</div>
