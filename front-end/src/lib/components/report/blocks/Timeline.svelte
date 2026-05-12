<script lang="ts">
	import type { TimelineEvent } from '$lib/api/types';

	interface Props {
		events: TimelineEvent[];
	}
	let { events }: Props = $props();

	let selectedIdx = $state<number | null>(null);

	function parseSeconds(ts: string): number {
		const m = ts.match(/T?\+?([\d.]+)\s*s?/i);
		return m ? parseFloat(m[1]) : 0;
	}

	function severityColor(s: string | undefined): string {
		switch ((s ?? '').toLowerCase()) {
			case 'critical': return '#dc2626';
			case 'high': return '#f87171';
			case 'medium': return '#fbbf24';
			case 'low': return '#94a3b8';
			default: return '#38bdf8';
		}
	}

	function severityBgClass(s: string | undefined): string {
		switch ((s ?? '').toLowerCase()) {
			case 'critical':
			case 'high': return 'bg-red-400/8';
			case 'medium': return 'bg-amber-400/8';
			default: return '';
		}
	}

	function severityLabel(s: string | undefined): string | null {
		if (!s) return null;
		return s.charAt(0).toUpperCase() + s.slice(1);
	}

	function severityBadgeClass(s: string | undefined): string {
		switch ((s ?? '').toLowerCase()) {
			case 'critical': return 'bg-red-500/15 text-red-300';
			case 'high': return 'bg-red-500/10 text-red-400';
			case 'medium': return 'bg-amber-500/10 text-amber-300';
			case 'low': return 'bg-slate-500/10 text-slate-300';
			default: return '';
		}
	}

	interface ParsedMeta {
		technique?: string;
		apis?: string[];
		rest: [string, string][];
	}

	function parseMeta(meta: unknown): ParsedMeta {
		if (meta == null || typeof meta !== 'object') return { rest: [] };
		const obj = meta as Record<string, unknown>;
		const technique = typeof obj.technique === 'string' ? obj.technique : undefined;
		let apis: string[] | undefined;
		if (Array.isArray(obj.apis)) apis = obj.apis.map(String);
		else if (typeof obj.api === 'string') apis = [obj.api];
		const skip = new Set(['technique', 'api', 'apis']);
		const rest: [string, string][] = [];
		for (const [k, v] of Object.entries(obj)) {
			if (skip.has(k) || v == null) continue;
			rest.push([k, Array.isArray(v) ? v.join(', ') : typeof v === 'object' ? JSON.stringify(v) : String(v)]);
		}
		return { technique, apis, rest };
	}

	function formatTime(s: number): string {
		if (s < 60) return `${s}s`;
		const m = Math.floor(s / 60);
		const rem = Math.round(s % 60);
		return rem > 0 ? `${m}m${rem}s` : `${m}m`;
	}

	const parsed = $derived(events.map((e, i) => ({
		event: e,
		idx: i,
		sec: parseSeconds(e.ts),
		color: severityColor(e.severity),
		meta: parseMeta(e.meta)
	})));

	const maxTime = $derived(Math.max(...parsed.map((e) => e.sec), 1));

	const ticks = $derived.by<number[]>(() => {
		const step = maxTime <= 15 ? 5 : maxTime <= 60 ? 10 : maxTime <= 180 ? 30 : 60;
		const out: number[] = [];
		for (let t = 0; t <= maxTime; t += step) out.push(t);
		return out;
	});

	const CARD_W = 240;
	const CARD_PAD = 8;
	const AXIS_H = 32;
	const STEM_H = 20;
	const ROW_H = 60;
	const MARGIN_L = 16;
	const MARGIN_R = 16;
	const BASE_W = 1200;
	const AXIS_W = BASE_W - MARGIN_L - MARGIN_R - CARD_W;

	type PlacedEvent = typeof parsed[number] & { x: number; row: number };

	function timeToX(sec: number): number {
		return MARGIN_L + (sec / maxTime) * AXIS_W;
	}

	const placed = $derived.by<PlacedEvent[]>(() => {
		if (parsed.length === 0) return [];
		const out: PlacedEvent[] = [];
		const rowEnds: number[] = [];

		for (const e of parsed) {
			const x = timeToX(e.sec);

			let row = 0;
			for (let r = 0; r < rowEnds.length; r++) {
				if (x >= rowEnds[r] + CARD_PAD) { row = r; break; }
				row = r + 1;
			}
			if (row >= rowEnds.length) rowEnds.push(0);
			rowEnds[row] = x + CARD_W;
			out.push({ ...e, x, row });
		}
		return out;
	});

	const maxRow = $derived(placed.length > 0 ? Math.max(...placed.map((e) => e.row)) : 0);

	const svgW = $derived.by(() => {
		if (placed.length === 0) return 1200;
		let maxRight = 0;
		for (const e of placed) {
			const right = e.x + CARD_W + MARGIN_R;
			if (right > maxRight) maxRight = right;
		}
		return Math.max(maxRight, 1200);
	});

	const svgH = $derived(AXIS_H + STEM_H + (maxRow + 1) * ROW_H + 16);
</script>

<div class="overflow-x-auto rounded-lg bg-[var(--color-bg-primary)]">
	<svg viewBox="0 0 {svgW} {svgH}" class="w-full" style="min-width: 800px;">
		<line x1={MARGIN_L} y1={AXIS_H} x2={MARGIN_L + AXIS_W} y2={AXIS_H}
			stroke="var(--color-border)" stroke-width="1" opacity="0.2" />

		{#each ticks as t (t)}
			{@const x = timeToX(t)}
			<line x1={x} y1={AXIS_H - 4} x2={x} y2={AXIS_H + 4}
				stroke="var(--color-border)" stroke-width="1" opacity="0.3" />
			<text x={x} y={AXIS_H - 10} text-anchor="middle"
				font-size="10" font-family="monospace"
				fill="var(--color-text-secondary)" opacity="0.45"
			>{formatTime(t)}</text>
		{/each}

		{#each placed as e (e.idx + '-stem')}
			{@const cardY = AXIS_H + STEM_H + e.row * ROW_H}
			<line x1={e.x} y1={AXIS_H + 4} x2={e.x} y2={cardY}
				stroke={e.color} stroke-width="1" opacity="0.2" />
		{/each}

		{#each placed as e (e.idx)}
			{@const cardY = AXIS_H + STEM_H + e.row * ROW_H}
			{@const isSel = selectedIdx === e.idx}
			{@const sev = severityLabel(e.event.severity)}

			<circle cx={e.x} cy={AXIS_H} r="3.5" fill={e.color} opacity="0.9" />

			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<!-- svelte-ignore a11y_click_events_have_key_events -->
			<g class="cursor-pointer" onclick={() => (selectedIdx = isSel ? null : e.idx)}>
				<rect x={e.x - 2} y={cardY} width={CARD_W} height={ROW_H - 8}
					rx="6" fill={isSel ? 'var(--color-bg-card)' : 'var(--color-bg-tertiary)'}
					stroke={isSel ? e.color : 'none'} stroke-width="1" stroke-opacity="0.4" />

				<rect x={e.x - 2} y={cardY} width="3" height={ROW_H - 8}
					rx="1.5" fill={e.color} opacity="0.7" />

				<text x={e.x + 10} y={cardY + 16} font-size="11" font-weight="500"
					fill="var(--color-text-primary)">
					{e.event.label.length > 32 ? e.event.label.slice(0, 30) + '...' : e.event.label}
				</text>

				<text x={e.x + 10} y={cardY + 32} font-size="9" font-family="monospace"
					fill="var(--color-text-secondary)" opacity="0.6">
					{e.event.ts}{sev ? ` · ${sev}` : ''}{e.meta.technique ? ` · ${e.meta.technique}` : ''}
				</text>

				{#if e.meta.apis && e.meta.apis.length > 0}
					<text x={e.x + 10} y={cardY + 44} font-size="8" font-family="monospace"
						fill="var(--color-text-secondary)" opacity="0.4">
						{e.meta.apis.slice(0, 3).join(', ')}{e.meta.apis.length > 3 ? ` +${e.meta.apis.length - 3}` : ''}
					</text>
				{/if}
			</g>
		{/each}
	</svg>
</div>

{#if selectedIdx !== null}
	{@const e = parsed[selectedIdx]}
	{@const sLabel = severityLabel(e.event.severity)}
	<div class="mt-3 rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3">
		<div class="flex items-center gap-2">
			<span class="size-2 shrink-0 rounded-full" style="background: {e.color}"></span>
			<span class="font-mono text-xs text-[var(--color-text-secondary)]">{e.event.ts}</span>
			<span class="text-sm text-[var(--color-text-primary)]">{e.event.label}</span>
			{#if sLabel}
				<span class="rounded px-1.5 py-0.5 text-[10px] font-medium leading-none {severityBadgeClass(e.event.severity)}">{sLabel}</span>
			{/if}
			{#if e.meta.technique}
				<a
					href="https://attack.mitre.org/techniques/{e.meta.technique.split('.')[0]}/"
					target="_blank"
					rel="noopener"
					class="rounded bg-[var(--color-accent)]/10 px-1.5 py-0.5 font-mono text-[10px] leading-none text-[var(--color-accent)] transition-colors hover:bg-[var(--color-accent)]/20"
				>{e.meta.technique}</a>
			{/if}
		</div>
		{#if (e.meta.apis && e.meta.apis.length > 0) || e.meta.rest.length > 0}
			<div class="mt-2 flex flex-wrap items-center gap-2">
				{#if e.meta.apis}
					{#each e.meta.apis as api (api)}
						<span class="rounded bg-[var(--color-bg-card)] px-1.5 py-0.5 font-mono text-[10px] text-[var(--color-text-secondary)]">{api}</span>
					{/each}
				{/if}
				{#each e.meta.rest as [k, v] (k)}
					<span class="text-[11px]">
						<span class="text-[var(--color-text-secondary)]">{k}</span>
						<span class="ml-1 font-mono text-[var(--color-text-primary)]/60">{v}</span>
					</span>
				{/each}
			</div>
		{/if}
	</div>
{/if}
