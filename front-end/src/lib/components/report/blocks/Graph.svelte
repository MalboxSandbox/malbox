<script lang="ts">
	import type { GraphEdge, GraphNode } from '$lib/api/types';

	interface Props {
		nodes: GraphNode[];
		edges: GraphEdge[];
	}
	let { nodes, edges }: Props = $props();

	let selectedNode = $state<string | null>(null);
	let hoveredEdge = $state<number | null>(null);

	function nodeType(meta: unknown): string {
		if (meta == null || typeof meta !== 'object') return 'default';
		return String((meta as Record<string, unknown>).type ?? 'default');
	}

	const TYPE_COLORS: Record<string, { accent: string; label: string }> = {
		malware: { accent: '#ef4444', label: 'Malware' },
		injected_process: { accent: '#f97316', label: 'Injected Process' },
		c2_server: { accent: '#a855f7', label: 'C2 Server' },
		c2_domain: { accent: '#8b5cf6', label: 'C2 Domain' },
		infrastructure: { accent: '#6366f1', label: 'Infrastructure' },
		target_process: { accent: '#eab308', label: 'Target Process' },
		persistence: { accent: '#f43f5e', label: 'Persistence' },
		default: { accent: '#6b7280', label: 'Node' }
	};

	function typeInfo(meta: unknown) {
		return TYPE_COLORS[nodeType(meta)] ?? TYPE_COLORS.default;
	}

	function metaEntries(meta: unknown): [string, string][] {
		if (meta == null || typeof meta !== 'object') return [];
		return Object.entries(meta as Record<string, unknown>)
			.filter(([k, v]) => v != null && k !== 'type')
			.map(([k, v]) => [k, String(v)]);
	}

	type Pos = { x: number; y: number };

	const NODE_W = 210;
	const NODE_H = 34;
	const ACCENT_W = 3;
	const LAYER_GAP = 270;
	const NODE_GAP = 56;
	const PAD_X = 24;
	const PAD_Y = 30;

	const layout = $derived.by(() => {
		const positions = new Map<string, Pos>();
		if (nodes.length === 0) return { positions, width: 0, height: 0 };

		const inDegree = new Map<string, number>();
		for (const n of nodes) inDegree.set(n.id, 0);
		for (const e of edges) inDegree.set(e.to, (inDegree.get(e.to) ?? 0) + 1);

		const roots = nodes.filter((n) => (inDegree.get(n.id) ?? 0) === 0).map((n) => n.id);
		if (roots.length === 0) roots.push(nodes[0].id);

		const layers = new Map<string, number>();
		const queue = [...roots];
		for (const r of roots) layers.set(r, 0);
		while (queue.length > 0) {
			const cur = queue.shift()!;
			const cl = layers.get(cur)!;
			for (const e of edges) {
				if (e.from === cur && !layers.has(e.to)) {
					layers.set(e.to, cl + 1);
					queue.push(e.to);
				}
			}
		}
		for (const n of nodes) {
			if (!layers.has(n.id)) layers.set(n.id, 0);
		}

		const layerGroups = new Map<number, string[]>();
		let maxLayer = 0;
		for (const [id, l] of layers) {
			if (!layerGroups.has(l)) layerGroups.set(l, []);
			layerGroups.get(l)!.push(id);
			if (l > maxLayer) maxLayer = l;
		}

		for (const [, ids] of layerGroups) {
			const l = layers.get(ids[0])!;
			for (let i = 0; i < ids.length; i++) {
				positions.set(ids[i], { x: PAD_X + l * LAYER_GAP, y: PAD_Y + i * NODE_GAP });
			}
		}

		const outEdges = new Map<string, string[]>();
		const inEdges = new Map<string, string[]>();
		for (const n of nodes) { outEdges.set(n.id, []); inEdges.set(n.id, []); }
		for (const e of edges) {
			outEdges.get(e.from)?.push(e.to);
			inEdges.get(e.to)?.push(e.from);
		}

		for (let iter = 0; iter < 80; iter++) {
			const forward = iter % 2 === 0;
			const layerOrder = [...layerGroups.keys()].sort((a, b) => forward ? a - b : b - a);

			for (const layer of layerOrder) {
				const ids = layerGroups.get(layer)!;
				if (ids.length <= 1) continue;

				for (const id of ids) {
					const p = positions.get(id)!;
					const nbrs = forward ? (outEdges.get(id) ?? []) : (inEdges.get(id) ?? []);
					const allNbrs = [...(outEdges.get(id) ?? []), ...(inEdges.get(id) ?? [])];
					const targets = nbrs.length > 0 ? nbrs : allNbrs;
					if (targets.length === 0) continue;

					let ty = 0;
					for (const nid of targets) { ty += positions.get(nid)!.y; }
					p.y += (ty / targets.length - p.y) * 0.4;
				}

				const sorted = [...ids].sort((a, b) => positions.get(a)!.y - positions.get(b)!.y);
				for (let j = 1; j < sorted.length; j++) {
					const prev = positions.get(sorted[j - 1])!;
					const curr = positions.get(sorted[j])!;
					if (curr.y - prev.y < NODE_GAP) curr.y = prev.y + NODE_GAP;
				}
			}
		}

		let minY = Infinity;
		for (const p of positions.values()) { if (p.y < minY) minY = p.y; }
		const shiftY = PAD_Y - minY;
		for (const p of positions.values()) { p.y += shiftY; }

		let maxX = 0;
		let maxY = 0;
		for (const p of positions.values()) {
			if (p.x + NODE_W + PAD_X > maxX) maxX = p.x + NODE_W + PAD_X;
			if (p.y + NODE_H + PAD_Y > maxY) maxY = p.y + NODE_H + PAD_Y;
		}

		return { positions, width: Math.max(maxX, 600), height: Math.max(maxY, 200) };
	});

	type EdgePath = {
		from: string; to: string; label?: string;
		path: string; idx: number;
		sx: number; sy: number; ex: number; ey: number;
		mx: number; my: number;
	};

	const edgePaths = $derived.by<EdgePath[]>(() => {
		const outPorts = new Map<string, { total: number; idx: number }>();
		const inPorts = new Map<string, { total: number; idx: number }>();
		for (const n of nodes) {
			outPorts.set(n.id, { total: edges.filter((e) => e.from === n.id).length, idx: 0 });
			inPorts.set(n.id, { total: edges.filter((e) => e.to === n.id).length, idx: 0 });
		}

		return edges.map((e, idx) => {
			const from = layout.positions.get(e.from);
			const to = layout.positions.get(e.to);
			if (!from || !to) return null;

			const op = outPorts.get(e.from)!;
			const ip = inPorts.get(e.to)!;
			const os = op.idx++;
			const is_ = ip.idx++;

			const oSpread = Math.min(NODE_H - 10, (op.total - 1) * 8);
			const iSpread = Math.min(NODE_H - 10, (ip.total - 1) * 8);
			const oOff = op.total <= 1 ? 0 : -oSpread / 2 + os * (oSpread / (op.total - 1));
			const iOff = ip.total <= 1 ? 0 : -iSpread / 2 + is_ * (iSpread / (ip.total - 1));

			const sx = from.x + NODE_W;
			const sy = from.y + NODE_H / 2 + oOff;
			const ex = to.x;
			const ey = to.y + NODE_H / 2 + iOff;

			const dx = ex - sx;
			const dy = Math.abs(ey - sy);
			const cp = Math.max(60, Math.min(dx * 0.45, 150));

			const path = `M ${sx} ${sy} C ${sx + cp} ${sy}, ${ex - cp} ${ey}, ${ex} ${ey}`;

			const t = 0.5;
			const mt = 1 - t;
			const cx1 = sx + cp;
			const cx2 = ex - cp;
			const mx = mt * mt * mt * sx + 3 * mt * mt * t * cx1 + 3 * mt * t * t * cx2 + t * t * t * ex;
			const my = mt * mt * mt * sy + 3 * mt * mt * t * sy + 3 * mt * t * t * ey + t * t * t * ey;

			return { from: e.from, to: e.to, label: e.label, path, idx, sx, sy, ex, ey, mx, my };
		}).filter(Boolean) as EdgePath[];
	});

	function isNodeActive(nodeId: string): boolean {
		if (!selectedNode) return true;
		if (nodeId === selectedNode) return true;
		return edges.some(
			(e) => (e.from === selectedNode && e.to === nodeId) || (e.to === selectedNode && e.from === nodeId)
		);
	}

	function isEdgeActive(e: EdgePath): boolean {
		if (!selectedNode) return true;
		return e.from === selectedNode || e.to === selectedNode;
	}

	function handleClick(id: string) {
		selectedNode = selectedNode === id ? null : id;
	}
</script>

<div class="space-y-3">
	<div class="overflow-x-auto rounded-lg bg-[var(--color-bg-primary)]">
		<svg
			viewBox="0 0 {layout.width} {layout.height}"
			class="w-full"
			style="min-width: 700px;"
			role="img"
			aria-label="Network communication graph"
		>
			<defs>
				<marker id="arr" markerWidth="7" markerHeight="5" refX="6" refY="2.5" orient="auto">
					<path d="M0 0.5 L6 2.5 L0 4.5Z" fill="var(--color-text-secondary)" opacity="0.2" />
				</marker>
				<marker id="arr-hl" markerWidth="7" markerHeight="5" refX="6" refY="2.5" orient="auto">
					<path d="M0 0.5 L6 2.5 L0 4.5Z" fill="var(--color-accent)" opacity="0.5" />
				</marker>
			</defs>

			{#each edgePaths as e (e.idx)}
				{@const hl = isEdgeActive(e)}
				{@const hovered = hoveredEdge === e.idx}
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<g
					onmouseenter={() => (hoveredEdge = e.idx)}
					onmouseleave={() => (hoveredEdge = null)}
				>
					<path
						d={e.path}
						fill="none"
						stroke="transparent"
						stroke-width="16"
						class="cursor-pointer"
					/>
					<path
						d={e.path}
						fill="none"
						stroke={hl ? 'var(--color-accent)' : 'var(--color-border)'}
						stroke-width={hovered ? 2 : hl ? 1.5 : 1}
						opacity={hovered ? 0.6 : hl ? 0.3 : 0.07}
						marker-end={hl ? 'url(#arr-hl)' : 'url(#arr)'}
					/>
				</g>
			{/each}

			{#each nodes as n (n.id)}
				{@const p = layout.positions.get(n.id)}
				{@const info = typeInfo(n.meta)}
				{@const active = isNodeActive(n.id)}
				{@const sel = selectedNode === n.id}
				{#if p}
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<!-- svelte-ignore a11y_click_events_have_key_events -->
					<g
						class="cursor-pointer"
						opacity={active ? 1 : 0.15}
						onclick={() => handleClick(n.id)}
					>
						<rect x={p.x} y={p.y} width={NODE_W} height={NODE_H} rx="6"
							fill={sel ? 'var(--color-bg-card)' : 'var(--color-bg-tertiary)'} />
						<rect x={p.x} y={p.y + 5} width={ACCENT_W} height={NODE_H - 10} rx="1.5"
							fill={info.accent} />
						{#if sel}
							<rect x={p.x} y={p.y} width={NODE_W} height={NODE_H} rx="6"
								fill="none" stroke={info.accent} stroke-width="1.5" opacity="0.4" />
						{/if}
						<text x={p.x + ACCENT_W + 12} y={p.y + NODE_H / 2 + 1}
							dominant-baseline="central" font-size="12"
							fill="var(--color-text-primary)">{n.label}</text>
					</g>
				{/if}
			{/each}

			{#each edgePaths as e (e.idx + '-label')}
				{@const show = hoveredEdge === e.idx || (selectedNode !== null && isEdgeActive(e))}
				{#if e.label && show}
					{@const textW = e.label.length * 6 + 12}
					<rect
						x={e.mx - textW / 2}
						y={e.my - 10}
						width={textW}
						height={16}
						rx="4"
						fill="var(--color-bg-card)"
						opacity="0.95"
					/>
					<text
						x={e.mx}
						y={e.my - 1}
						text-anchor="middle"
						dominant-baseline="central"
						font-size="10"
						fill="var(--color-text-primary)"
					>{e.label}</text>
				{/if}
			{/each}
		</svg>
	</div>

	{#if selectedNode}
		{@const node = nodes.find((n) => n.id === selectedNode)}
		{#if node}
			{@const entries = metaEntries(node.meta)}
			{@const info = typeInfo(node.meta)}
			{@const outgoing = edges.filter((e) => e.from === selectedNode)}
			{@const incoming = edges.filter((e) => e.to === selectedNode)}
			<div class="rounded-lg bg-[var(--color-bg-tertiary)] px-3 py-2.5 text-xs">
				<div class="flex items-center gap-2">
					<span class="size-2 shrink-0 rounded-sm" style="background: {info.accent}"></span>
					<span class="font-medium text-[var(--color-text-primary)]">{node.label}</span>
					<span class="text-[10px] text-[var(--color-text-secondary)]">{info.label}</span>
				</div>
				{#if entries.length > 0}
					<div class="mt-1.5 flex flex-wrap gap-x-4 gap-y-1">
						{#each entries as [k, v] (k)}
							<span>
								<span class="text-[var(--color-text-secondary)]">{k}</span>
								<span class="ml-1 font-mono text-[var(--color-text-primary)]">{v}</span>
							</span>
						{/each}
					</div>
				{/if}
				{#if outgoing.length > 0 || incoming.length > 0}
					<div class="mt-1.5 flex flex-wrap gap-x-3 gap-y-1 text-[11px] text-[var(--color-text-secondary)]">
						{#each outgoing as e (e.to)}
							{@const t = nodes.find((nn) => nn.id === e.to)}
							<span>
								<span class="text-[var(--color-accent)]">&rarr;</span>
								{t?.label ?? e.to}
								{#if e.label}<span class="opacity-50">({e.label})</span>{/if}
							</span>
						{/each}
						{#each incoming as e (e.from)}
							{@const s = nodes.find((nn) => nn.id === e.from)}
							<span>
								<span class="text-[var(--color-accent)]">&larr;</span>
								{s?.label ?? e.from}
								{#if e.label}<span class="opacity-50">({e.label})</span>{/if}
							</span>
						{/each}
					</div>
				{/if}
			</div>
		{/if}
	{/if}

	<div class="flex flex-wrap gap-x-4 gap-y-1 text-[10px]">
		{#each Object.entries(TYPE_COLORS) as [type, info] (type)}
			{#if type !== 'default' && nodes.some((n) => nodeType(n.meta) === type)}
				<span class="flex items-center gap-1.5 text-[var(--color-text-secondary)]">
					<span class="size-1.5 rounded-sm" style="background: {info.accent}"></span>
					{info.label}
				</span>
			{/if}
		{/each}
	</div>
</div>
