<script lang="ts">
	import { splitDateTime } from '$lib/api/format';
	import type { Column } from '$lib/api/types';

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
		if (value == null) return '—';
		switch (type) {
			case 'datetime': {
				const s = String(value);
				const { date, time } = splitDateTime(s);
				return time ? `${date} ${time}` : date;
			}
			case 'number':
				return typeof value === 'number'
					? new Intl.NumberFormat().format(value)
					: String(value);
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

	const cellAlign = (type: string | undefined) =>
		type === 'number' ? 'text-right' : 'text-left';
</script>

<div class="space-y-3">
	{#if searchable}
		<input
			type="search"
			bind:value={query}
			placeholder="Search…"
			class="w-full max-w-xs rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-3 py-1.5 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-1 focus:ring-[var(--color-accent)]"
		/>
	{/if}

	<div class="overflow-x-auto rounded-lg border border-[var(--color-border)]">
		<table class="w-full text-sm">
			<thead class="bg-[var(--color-bg-tertiary)] text-xs uppercase text-[var(--color-text-secondary)]">
				<tr>
					{#each columns as c (c.key)}
						<th class="px-4 py-2 font-medium {cellAlign(c.type)}">
							{#if sortable}
								<button
									type="button"
									class="inline-flex items-center gap-1 hover:text-[var(--color-text-primary)]"
									onclick={() => toggleSort(c.key)}
								>
									<span>{c.label}</span>
									{#if sortKey === c.key}
										<span>{sortDir === 'asc' ? '↑' : '↓'}</span>
									{/if}
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
						class="border-t border-[var(--color-border)] text-[var(--color-text-primary)] hover:bg-[var(--color-bg-tertiary)]/50"
					>
						{#each columns as c (c.key)}
							<td class="px-4 py-2 {cellAlign(c.type)} {c.type === 'string' || !c.type ? 'break-all' : ''}">
								{formatCell(row[c.key], c.type)}
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
