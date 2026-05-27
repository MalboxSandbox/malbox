<script lang="ts">
	import { splitDateTime } from '$lib/api/format';
	import type { Column } from '$lib/api/types';
	import { contextmenu } from '$lib/actions/contextmenu';
	import { truncateWithTail } from '$lib/utils/truncate';

	interface Props {
		columns: Column[];
		rows: Record<string, unknown>[];
		sortable?: boolean;
		searchable?: boolean;
	}
	let { columns, rows, sortable = false, searchable = false }: Props = $props();

	let query = $state('');
	let sortKey = $state<string | null>(null);
	let sortDir = $state<'asc' | 'desc'>('asc');
	let copiedCell = $state<string | null>(null);

	async function copyCell(value: unknown, type: string | undefined, id: string) {
		if (window.getSelection()?.toString()) return;
		await navigator.clipboard.writeText(formatCell(value, type));
		copiedCell = id;
		setTimeout(() => (copiedCell = null), 1500);
	}

	function toggleSort(k: string) {
		if (!sortable) return;
		if (sortKey === k) {
			sortDir = sortDir === 'asc' ? 'desc' : 'asc';
		} else {
			sortKey = k;
			sortDir = 'asc';
		}
	}

	function compare(a: unknown, b: unknown): number {
		if (a == null && b == null) return 0;
		if (a == null) return -1;
		if (b == null) return 1;
		if (typeof a === 'number' && typeof b === 'number') return a - b;
		return String(a).localeCompare(String(b));
	}

	function formatCell(value: unknown, type: string | undefined): string {
		if (value == null) return '-';
		switch (type) {
			case 'datetime': {
				const s = String(value);
				const { date, time } = splitDateTime(s);
				return time ? `${date} ${time}` : date;
			}
			case 'number':
				return typeof value === 'number' ? new Intl.NumberFormat().format(value) : String(value);
			case 'bool':
				return value ? '✓' : '✗';
			default:
				return typeof value === 'object' ? JSON.stringify(value) : String(value);
		}
	}

	const filtered = $derived.by(() => {
		if (!searchable || query.trim() === '') return rows;
		const q = query.toLowerCase();
		return rows.filter((r) =>
			columns.some((c) => {
				const v = r[c.key];
				if (v == null) return false;
				return String(v).toLowerCase().includes(q);
			})
		);
	});

	const sorted = $derived.by(() => {
		if (!sortable || sortKey == null) return filtered;
		const key = sortKey;
		const dir = sortDir === 'asc' ? 1 : -1;
		return [...filtered].sort((a, b) => compare(a[key], b[key]) * dir);
	});

	const cellAlign = (type: string | undefined) => (type === 'number' ? 'text-right' : 'text-left');
</script>

<div class="max-w-full space-y-3 overflow-hidden">
	{#if searchable}
		<input
			type="search"
			bind:value={query}
			placeholder="Search..."
			class="w-full max-w-xs rounded-lg bg-[var(--color-bg-tertiary)] px-3 py-1.5 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-1 focus:ring-[var(--color-accent)]"
		/>
	{/if}

	<div class="max-h-[32rem] overflow-auto rounded-lg bg-[var(--color-bg-primary)]">
		<table class="w-full text-sm">
			<thead class="sticky top-0 z-10 bg-[var(--color-bg-primary)]">
				<tr class="text-xs uppercase text-[var(--color-text-secondary)]">
					{#each columns as c (c.key)}
						<th class="px-4 py-2.5 font-medium {cellAlign(c.type)}">
							{#if sortable}
								<button
									type="button"
									class="inline-flex items-center gap-1.5 transition-colors hover:text-[var(--color-text-primary)]"
									onclick={() => toggleSort(c.key)}
								>
									<span>{c.label}</span>
									<svg
										xmlns="http://www.w3.org/2000/svg"
										viewBox="0 0 16 16"
										fill="currentColor"
										class="size-3 transition-transform {sortKey === c.key
											? 'text-[var(--color-text-primary)]'
											: 'opacity-0'} {sortKey === c.key && sortDir === 'desc' ? 'rotate-180' : ''}"
									>
										<path
											fill-rule="evenodd"
											d="M8 3.5a.75.75 0 0 1 .75.75v6.19l2.72-2.72a.75.75 0 1 1 1.06 1.06l-4 4a.75.75 0 0 1-1.06 0l-4-4a.75.75 0 0 1 1.06-1.06l2.72 2.72V4.25A.75.75 0 0 1 8 3.5Z"
											clip-rule="evenodd"
										/>
									</svg>
								</button>
							{:else}
								{c.label}
							{/if}
						</th>
					{/each}
				</tr>
			</thead>
			<tbody>
				{#each sorted as row, i (i)}
					<tr
						class="border-t border-[var(--color-border)]/30 text-[var(--color-text-primary)] transition-colors hover:bg-[var(--color-bg-tertiary)]/40"
					>
						{#each columns as c (c.key)}
							{@const formatted = formatCell(row[c.key], c.type)}
							{@const parts =
								c.type === 'string' || !c.type
									? truncateWithTail(formatted, 150, 15)
									: { head: formatted, tail: '', truncated: false }}
							<td
								class="px-4 py-2 {cellAlign(c.type)} cursor-copy"
								onclick={() => copyCell(row[c.key], c.type, `${i}-${c.key}`)}
								use:contextmenu={{
									type: 'table-cell',
									value: formatted,
									metadata: { row, label: c.label }
								}}
							>
								<span
									class="transition-colors {c.type === 'string' || !c.type
										? 'break-all'
										: ''} {copiedCell === `${i}-${c.key}` ? 'text-[var(--color-accent)]' : ''}"
								>
									{parts.head}{#if parts.truncated}<span class="text-[var(--color-text-secondary)]"
											>…</span
										>{parts.tail}{/if}
								</span>
							</td>
						{/each}
					</tr>
				{/each}
				{#if sorted.length === 0}
					<tr>
						<td
							class="px-4 py-6 text-center text-[var(--color-text-secondary)]"
							colspan={columns.length}
						>
							{searchable && query ? 'No matching rows.' : 'No rows.'}
						</td>
					</tr>
				{/if}
			</tbody>
		</table>
	</div>
</div>
