<script lang="ts">
	import CopyButton from '$lib/components/ui/CopyButton.svelte';
	import { SearchIcon, CornerDownRightIcon } from '@lucide/svelte';
	import { contextMenuStore } from '$lib/stores/contextMenu.svelte';
	import { resolveMenuItems } from '$lib/context-menu/registry';
	import type { ContextPayload } from '$lib/components/context-menu/types';

	interface Props {
		bytes_b64: string;
		offset?: number;
	}
	let { bytes_b64, offset = 0 }: Props = $props();

	function decode(b64: string): Uint8Array {
		try {
			const bin = atob(b64);
			const out = new Uint8Array(bin.length);
			for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
			return out;
		} catch {
			return new Uint8Array();
		}
	}

	const bytes = $derived(decode(bytes_b64));

	function hex2(n: number): string {
		return n.toString(16).padStart(2, '0');
	}
	function offsetHex(n: number): string {
		return n.toString(16).padStart(8, '0');
	}

	const rows = $derived.by(() => {
		const out: { offset: number; offHex: string; byteIndices: number[] }[] = [];
		for (let i = 0; i < bytes.length; i += 16) {
			const indices: number[] = [];
			for (let j = i; j < Math.min(i + 16, bytes.length); j++) {
				indices.push(j);
			}
			out.push({ offset: offset + i, offHex: offsetHex(offset + i), byteIndices: indices });
		}
		return out;
	});

	const fullHexText = $derived.by(() => {
		return Array.from(bytes)
			.map((b) => hex2(b))
			.join(' ');
	});

	let hoveredOffset = $state<number | null>(null);

	let showSearch = $state(false);
	let showGoto = $state(false);

	// --- Selection state for data inspector (Task 7) ---
	let selectionStart = $state<number | null>(null);
	let selectionEnd = $state<number | null>(null);

	const selectedRange = $derived.by((): { start: number; end: number } | null => {
		if (selectionStart === null) return null;
		const s = selectionEnd !== null ? Math.min(selectionStart, selectionEnd) : selectionStart;
		const e = selectionEnd !== null ? Math.max(selectionStart, selectionEnd) : selectionStart;
		return { start: s, end: e };
	});

	function handleByteClick(idx: number, e: MouseEvent | KeyboardEvent) {
		if (e.shiftKey && selectionStart !== null) {
			selectionEnd = idx;
		} else {
			selectionStart = idx;
			selectionEnd = null;
		}
	}

	function isSelected(idx: number): boolean {
		if (!selectedRange) return false;
		return idx >= selectedRange.start && idx <= selectedRange.end;
	}

	function asciiChar(b: number): string {
		return b >= 32 && b < 127 ? String.fromCharCode(b) : '.';
	}

	function hexSep(j: number): string {
		return j === 7 ? '  ' : ' ';
	}

	// --- Search state (Task 6) ---
	let searchQuery = $state('');
	let searchMode = $state<'hex' | 'ascii'>('hex');
	let searchCurrentMatch = $state(0);

	interface ByteMatch {
		start: number;
		length: number;
	}

	let searchMatches = $state<ByteMatch[]>([]);

	function isSearchMatch(idx: number): boolean {
		return searchMatches.some((m) => idx >= m.start && idx < m.start + m.length);
	}

	function isCurrentSearchMatch(idx: number): boolean {
		if (searchMatches.length === 0 || searchCurrentMatch >= searchMatches.length) return false;
		const m = searchMatches[searchCurrentMatch];
		return idx >= m.start && idx < m.start + m.length;
	}

	function parseHexQuery(q: string): number[] | null {
		const cleaned = q.replace(/\s+/g, '');
		if (cleaned.length === 0 || cleaned.length % 2 !== 0) return null;
		if (!/^[0-9a-fA-F]+$/.test(cleaned)) return null;
		const result: number[] = [];
		for (let i = 0; i < cleaned.length; i += 2) {
			result.push(parseInt(cleaned.slice(i, i + 2), 16));
		}
		return result;
	}

	function searchBytes(pattern: number[]): ByteMatch[] {
		const results: ByteMatch[] = [];
		for (let i = 0; i <= bytes.length - pattern.length; i++) {
			let match = true;
			for (let j = 0; j < pattern.length; j++) {
				if (bytes[i + j] !== pattern[j]) {
					match = false;
					break;
				}
			}
			if (match) results.push({ start: i, length: pattern.length });
		}
		return results;
	}

	function runSearch() {
		if (!searchQuery.trim()) {
			searchMatches = [];
			searchCurrentMatch = 0;
			return;
		}
		if (searchMode === 'hex') {
			const pattern = parseHexQuery(searchQuery);
			if (!pattern) {
				searchMatches = [];
				return;
			}
			searchMatches = searchBytes(pattern);
		} else {
			const asciiBytes = Array.from(new TextEncoder().encode(searchQuery));
			searchMatches = searchBytes(asciiBytes);
		}
		searchCurrentMatch = 0;
	}

	function searchNext() {
		if (searchMatches.length === 0) return;
		searchCurrentMatch = (searchCurrentMatch + 1) % searchMatches.length;
		scrollToMatch(searchCurrentMatch);
	}

	function searchPrev() {
		if (searchMatches.length === 0) return;
		searchCurrentMatch = (searchCurrentMatch - 1 + searchMatches.length) % searchMatches.length;
		scrollToMatch(searchCurrentMatch);
	}

	function scrollToMatch(idx: number) {
		if (!hexAreaEl || idx >= searchMatches.length) return;
		const matchOffset = searchMatches[idx].start;
		const rowIndex = Math.floor(matchOffset / 16);
		const lineHeight = 20;
		const targetTop = rowIndex * lineHeight;
		hexAreaEl.scrollTop = Math.max(0, targetTop - hexAreaEl.clientHeight / 2);
	}

	function formatHexInput(value: string): string {
		const cleaned = value.replace(/[^0-9a-fA-F]/g, '');
		const pairs: string[] = [];
		for (let i = 0; i < cleaned.length; i += 2) {
			pairs.push(cleaned.slice(i, i + 2));
		}
		return pairs.join(' ').toUpperCase();
	}

	function handleSearchKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			showSearch = false;
			searchMatches = [];
			searchQuery = '';
		} else if (e.key === 'Enter' && e.shiftKey) {
			e.preventDefault();
			searchPrev();
		} else if (e.key === 'Enter') {
			e.preventDefault();
			if (searchMatches.length === 0) runSearch();
			else searchNext();
		}
	}

	// --- Go-to state (Task 6) ---
	let gotoInput = $state('');
	let gotoError = $state(false);
	let hexAreaEl: HTMLDivElement | undefined = $state();
	let highlightedRow = $state<number | null>(null);

	function handleGoto() {
		const input = gotoInput.trim();
		if (!input) return;
		let targetOffset: number;
		if (input.startsWith('0x') || input.startsWith('0X')) {
			targetOffset = parseInt(input.slice(2), 16);
		} else if (/^[0-9a-fA-F]+$/.test(input) && /[a-fA-F]/.test(input)) {
			targetOffset = parseInt(input, 16);
		} else {
			targetOffset = parseInt(input, 10);
		}
		if (isNaN(targetOffset) || targetOffset < offset || targetOffset >= offset + bytes.length) {
			gotoError = true;
			setTimeout(() => (gotoError = false), 1500);
			return;
		}
		const relativeOffset = targetOffset - offset;
		const rowIndex = Math.floor(relativeOffset / 16);
		if (!hexAreaEl) return;
		const lineHeight = 20;
		hexAreaEl.scrollTop = Math.max(0, rowIndex * lineHeight - hexAreaEl.clientHeight / 2);
		highlightedRow = rowIndex;
		setTimeout(() => (highlightedRow = null), 1500);
		showGoto = false;
		gotoInput = '';
	}

	function handleGotoKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			showGoto = false;
			gotoInput = '';
		} else if (e.key === 'Enter') {
			e.preventDefault();
			handleGoto();
		}
	}

	function handleCodeContextMenu(e: MouseEvent) {
		const selection = window.getSelection()?.toString();
		if (!selection) return;
		e.preventDefault();
		e.stopPropagation();
		const payload: ContextPayload = { type: 'code-block', value: selection };
		const items = resolveMenuItems(payload);
		if (items.length > 0) {
			contextMenuStore.openMenu({ x: e.clientX, y: e.clientY }, payload, items);
		}
	}

	function byteClass(idx: number): string {
		const parts = ['cursor-pointer', 'transition-colors', 'duration-75'];
		if (isSelected(idx)) {
			parts.push('bg-[var(--color-accent)]/30 text-[var(--color-text-primary)]');
		} else if (isCurrentSearchMatch(idx)) {
			parts.push('bg-amber-400/60 text-[var(--color-bg-primary)]');
		} else if (isSearchMatch(idx)) {
			parts.push('bg-amber-400/30');
		} else if (hoveredOffset === idx) {
			parts.push('bg-[var(--color-bg-card)]');
		}
		return parts.join(' ');
	}

	const selectedBytes = $derived.by((): Uint8Array => {
		if (!selectedRange) return new Uint8Array();
		return bytes.slice(selectedRange.start, selectedRange.end + 1);
	});

	interface Interpretation {
		label: string;
		value: string;
	}

	const interpretations = $derived.by((): Interpretation[] => {
		const sel = selectedBytes;
		if (sel.length === 0) return [];

		const view = new DataView(sel.buffer, sel.byteOffset, sel.byteLength);
		const results: Interpretation[] = [];

		if (sel.length >= 1) {
			results.push({ label: 'uint8', value: view.getUint8(0).toString() });
			results.push({ label: 'int8', value: view.getInt8(0).toString() });
		}
		if (sel.length >= 2) {
			results.push({ label: 'uint16 LE', value: view.getUint16(0, true).toString() });
			results.push({ label: 'uint16 BE', value: view.getUint16(0, false).toString() });
			results.push({ label: 'int16 LE', value: view.getInt16(0, true).toString() });
			results.push({ label: 'int16 BE', value: view.getInt16(0, false).toString() });
		}
		if (sel.length >= 4) {
			results.push({ label: 'uint32 LE', value: view.getUint32(0, true).toString() });
			results.push({ label: 'uint32 BE', value: view.getUint32(0, false).toString() });
			results.push({ label: 'int32 LE', value: view.getInt32(0, true).toString() });
			results.push({ label: 'int32 BE', value: view.getInt32(0, false).toString() });
			results.push({
				label: 'float32 LE',
				value: view.getFloat32(0, true).toPrecision(6)
			});
			results.push({
				label: 'float32 BE',
				value: view.getFloat32(0, false).toPrecision(6)
			});

			const unixTs = view.getUint32(0, true);
			const unixDate = new Date(unixTs * 1000);
			results.push({
				label: 'Unix timestamp',
				value:
					unixTs > 0 && unixTs < 4294967295 && !isNaN(unixDate.getTime())
						? unixDate.toISOString()
						: 'Invalid'
			});
		}
		if (sel.length >= 8) {
			const u64le = view.getBigUint64(0, true);
			const u64be = view.getBigUint64(0, false);
			const i64le = view.getBigInt64(0, true);
			const i64be = view.getBigInt64(0, false);
			const safe = BigInt(Number.MAX_SAFE_INTEGER);
			results.push({
				label: 'uint64 LE',
				value: u64le > safe ? '0x' + u64le.toString(16) : u64le.toString()
			});
			results.push({
				label: 'uint64 BE',
				value: u64be > safe ? '0x' + u64be.toString(16) : u64be.toString()
			});
			results.push({
				label: 'int64 LE',
				value: i64le > safe || i64le < -safe ? '0x' + i64le.toString(16) : i64le.toString()
			});
			results.push({
				label: 'int64 BE',
				value: i64be > safe || i64be < -safe ? '0x' + i64be.toString(16) : i64be.toString()
			});
			results.push({
				label: 'float64 LE',
				value: view.getFloat64(0, true).toPrecision(10)
			});
			results.push({
				label: 'float64 BE',
				value: view.getFloat64(0, false).toPrecision(10)
			});

			const ftLE = view.getBigUint64(0, true);
			const ftEpochDiff = BigInt('116444736000000000');
			if (ftLE > ftEpochDiff) {
				const microseconds = (ftLE - ftEpochDiff) / BigInt(10);
				const ms = Number(microseconds / BigInt(1000));
				const ftDate = new Date(ms);
				results.push({
					label: 'FILETIME',
					value: !isNaN(ftDate.getTime()) ? ftDate.toISOString() : 'Invalid'
				});
			} else {
				results.push({ label: 'FILETIME', value: 'Invalid' });
			}
		}

		const utf8 = formatInspectorString(new TextDecoder('utf-8', { fatal: false }).decode(sel));
		results.push({ label: 'UTF-8', value: utf8 });

		if (sel.length >= 2) {
			const utf16 = formatInspectorString(
				new TextDecoder('utf-16le', { fatal: false }).decode(sel)
			);
			results.push({ label: 'UTF-16 LE', value: utf16 });
		}

		return results;
	});

	function formatInspectorString(s: string): string {
		return s
			.split('')
			.map((c) => {
				const code = c.charCodeAt(0);
				if (code < 32 || code === 127) return '\\x' + code.toString(16).padStart(2, '0');
				return c;
			})
			.join('');
	}
</script>

<div
	class="overflow-hidden rounded-lg bg-[var(--color-bg-primary)]"
	oncontextmenu={handleCodeContextMenu}
	role="application"
>
	<!-- Toolbar -->
	<div class="flex items-center justify-between px-4 py-2">
		<span class="font-mono text-[10px] uppercase text-[var(--color-text-secondary)]">
			hex &middot; {bytes.length} bytes
		</span>
		<div class="flex items-center gap-1.5">
			<button
				type="button"
				class="rounded p-1 transition-colors {showSearch
					? 'text-[var(--color-accent)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				onclick={() => {
					showSearch = !showSearch;
					if (showSearch) showGoto = false;
				}}
				title="Search hex"
			>
				<SearchIcon class="size-3.5" />
			</button>
			<button
				type="button"
				class="rounded p-1 transition-colors {showGoto
					? 'text-[var(--color-accent)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				onclick={() => {
					showGoto = !showGoto;
					if (showGoto) showSearch = false;
				}}
				title="Go to offset"
			>
				<CornerDownRightIcon class="size-3.5" />
			</button>
			<CopyButton value={fullHexText} size="sm" />
		</div>
	</div>

	{#if showSearch}
		<div class="flex items-center gap-2 border-t border-[var(--color-border)]/30 px-4 py-2">
			<div class="flex items-center gap-1 rounded bg-[var(--color-bg-card)] p-0.5">
				<button
					type="button"
					class="rounded px-1.5 py-0.5 text-[10px] transition-colors {searchMode === 'hex'
						? 'bg-[var(--color-accent)]/20 text-[var(--color-accent)]'
						: 'text-[var(--color-text-secondary)]'}"
					onclick={() => {
						searchMode = 'hex';
						runSearch();
					}}>Hex</button
				>
				<button
					type="button"
					class="rounded px-1.5 py-0.5 text-[10px] transition-colors {searchMode === 'ascii'
						? 'bg-[var(--color-accent)]/20 text-[var(--color-accent)]'
						: 'text-[var(--color-text-secondary)]'}"
					onclick={() => {
						searchMode = 'ascii';
						runSearch();
					}}>ASCII</button
				>
			</div>
			<input
				type="text"
				placeholder={searchMode === 'hex' ? '4D 5A 90...' : 'Search text...'}
				value={searchMode === 'hex' ? formatHexInput(searchQuery) : searchQuery}
				oninput={(e) => {
					searchQuery = (e.currentTarget as HTMLInputElement).value;
					runSearch();
				}}
				onkeydown={handleSearchKeydown}
				class="min-w-0 flex-1 bg-transparent font-mono text-xs text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none"
			/>
			<span class="shrink-0 text-[10px] text-[var(--color-text-secondary)]">
				{#if searchQuery && searchMatches.length > 0}
					{searchCurrentMatch + 1} of {searchMatches.length}
				{:else if searchQuery}
					No results
				{/if}
			</span>
			<button
				type="button"
				class="text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] disabled:opacity-30"
				onclick={searchPrev}
				disabled={searchMatches.length === 0}
				title="Previous"
			>
				<svg class="size-3.5" viewBox="0 0 20 20" fill="currentColor">
					<path
						fill-rule="evenodd"
						d="M14.77 12.79a.75.75 0 01-1.06-.02L10 8.832 6.29 12.77a.75.75 0 11-1.08-1.04l4.25-4.5a.75.75 0 011.08 0l4.25 4.5a.75.75 0 01-.02 1.06z"
						clip-rule="evenodd"
					/>
				</svg>
			</button>
			<button
				type="button"
				class="text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] disabled:opacity-30"
				onclick={searchNext}
				disabled={searchMatches.length === 0}
				title="Next"
			>
				<svg class="size-3.5" viewBox="0 0 20 20" fill="currentColor">
					<path
						fill-rule="evenodd"
						d="M5.23 7.21a.75.75 0 011.06.02L10 11.168l3.71-3.938a.75.75 0 111.08 1.04l-4.25 4.5a.75.75 0 01-1.08 0l-4.25-4.5a.75.75 0 01.02-1.06z"
						clip-rule="evenodd"
					/>
				</svg>
			</button>
			<button
				type="button"
				class="text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]"
				onclick={() => {
					showSearch = false;
					searchMatches = [];
					searchQuery = '';
				}}
				title="Close"
			>
				<svg class="size-3.5" viewBox="0 0 20 20" fill="currentColor">
					<path
						d="M6.28 5.22a.75.75 0 00-1.06 1.06L8.94 10l-3.72 3.72a.75.75 0 101.06 1.06L10 11.06l3.72 3.72a.75.75 0 101.06-1.06L11.06 10l3.72-3.72a.75.75 0 00-1.06-1.06L10 8.94 6.28 5.22z"
					/>
				</svg>
			</button>
		</div>
	{/if}

	{#if showGoto}
		<div class="flex items-center gap-2 border-t border-[var(--color-border)]/30 px-4 py-2">
			<span class="text-[10px] text-[var(--color-text-secondary)]">Go to:</span>
			<input
				type="text"
				bind:value={gotoInput}
				onkeydown={handleGotoKeydown}
				placeholder="0x1A40 or 6720"
				class="w-32 bg-transparent font-mono text-xs text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none {gotoError
					? 'text-[var(--color-error)]'
					: ''}"
			/>
			{#if gotoError}
				<span class="text-[10px] text-[var(--color-error)]">Out of range</span>
			{/if}
			<button
				type="button"
				class="text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]"
				onclick={() => {
					showGoto = false;
					gotoInput = '';
				}}
				title="Close"
			>
				<svg class="size-3.5" viewBox="0 0 20 20" fill="currentColor">
					<path
						d="M6.28 5.22a.75.75 0 00-1.06 1.06L8.94 10l-3.72 3.72a.75.75 0 101.06 1.06L10 11.06l3.72 3.72a.75.75 0 101.06-1.06L11.06 10l3.72-3.72a.75.75 0 00-1.06-1.06L10 8.94 6.28 5.22z"
					/>
				</svg>
			</button>
		</div>
	{/if}

	<!-- Hex dump -->
	<div
		bind:this={hexAreaEl}
		class="overflow-x-auto border-t border-[var(--color-border)]/30 px-4 py-3 font-mono text-xs leading-5"
		style="max-height: 500px; overflow-y: auto;"
	>
		{#each rows as row, rowIdx (row.offset)}
			<div
				class="flex {highlightedRow === rowIdx ? 'animate-pulse bg-[var(--color-accent)]/10' : ''}"
			>
				<!-- Offset column -->
				<span class="mr-4 select-none text-[var(--color-text-secondary)]">{row.offHex}</span>
				<!-- Hex column -->
				<span class="mr-4">
					{#each row.byteIndices as idx, j (idx)}
						<span
							class={byteClass(idx)}
							onmouseenter={() => (hoveredOffset = idx)}
							onmouseleave={() => (hoveredOffset = null)}
							onclick={(e) => handleByteClick(idx, e)}
							onkeydown={(e) => {
								if (e.key === 'Enter' || e.key === ' ') handleByteClick(idx, e);
							}}
							role="button"
							tabindex="-1">{hex2(bytes[idx])}</span
						>{#if j < row.byteIndices.length - 1}{hexSep(j)}{/if}
					{/each}
					{#if row.byteIndices.length < 16}
						<span class="select-none"
							>{' '.repeat(
								(16 - row.byteIndices.length) * 3 + (row.byteIndices.length <= 8 ? 1 : 0)
							)}</span
						>
					{/if}
				</span>
				<!-- ASCII column -->
				<span>
					{#each row.byteIndices as idx (idx)}
						<span
							class="{byteClass(idx)} {!isSelected(idx) &&
							!isSearchMatch(idx) &&
							hoveredOffset !== idx
								? 'text-[var(--color-accent)]/60'
								: ''}"
							onmouseenter={() => (hoveredOffset = idx)}
							onmouseleave={() => (hoveredOffset = null)}
							onclick={(e) => handleByteClick(idx, e)}
							onkeydown={(e) => {
								if (e.key === 'Enter' || e.key === ' ') handleByteClick(idx, e);
							}}
							role="button"
							tabindex="-1">{asciiChar(bytes[idx])}</span
						>
					{/each}
				</span>
			</div>
		{/each}
	</div>

	{#if selectedRange}
		<div class="border-t border-[var(--color-border)]/30 px-4 py-3">
			<div class="mb-2 flex items-center justify-between">
				<span class="font-mono text-[10px] uppercase text-[var(--color-text-secondary)]">
					Inspector &middot; {selectedBytes.length} byte{selectedBytes.length !== 1 ? 's' : ''} at 0x{offsetHex(
						offset + selectedRange.start
					)}
				</span>
				<button
					type="button"
					class="text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text-primary)]"
					onclick={() => {
						selectionStart = null;
						selectionEnd = null;
					}}
					title="Clear selection"
				>
					<svg class="size-3.5" viewBox="0 0 20 20" fill="currentColor">
						<path
							d="M6.28 5.22a.75.75 0 00-1.06 1.06L8.94 10l-3.72 3.72a.75.75 0 101.06 1.06L10 11.06l3.72 3.72a.75.75 0 101.06-1.06L11.06 10l3.72-3.72a.75.75 0 00-1.06-1.06L10 8.94 6.28 5.22z"
						/>
					</svg>
				</button>
			</div>
			<div class="grid grid-cols-2 gap-x-6 gap-y-1">
				{#each interpretations as interp (interp.label)}
					<div class="flex items-center justify-between gap-2 py-0.5">
						<span class="shrink-0 font-mono text-[10px] text-[var(--color-text-secondary)]">
							{interp.label}
						</span>
						<div class="flex items-center gap-1">
							<span
								class="truncate font-mono text-xs text-[var(--color-text-primary)]"
								title={interp.value}
							>
								{interp.value}
							</span>
							<button
								type="button"
								class="shrink-0 text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text-primary)]"
								onclick={() => navigator.clipboard.writeText(interp.value)}
								title="Copy value"
							>
								<svg class="size-3" viewBox="0 0 20 20" fill="currentColor">
									<path
										d="M7 3.5A1.5 1.5 0 0 1 8.5 2h3.879a1.5 1.5 0 0 1 1.06.44l3.122 3.12A1.5 1.5 0 0 1 17 6.622V12.5a1.5 1.5 0 0 1-1.5 1.5h-1v-3.379a3 3 0 0 0-.879-2.121L10.5 5.379A3 3 0 0 0 8.379 4.5H7v-1Z"
									/>
									<path
										d="M4.5 6A1.5 1.5 0 0 0 3 7.5v9A1.5 1.5 0 0 0 4.5 18h7a1.5 1.5 0 0 0 1.5-1.5v-5.879a1.5 1.5 0 0 0-.44-1.06L9.44 6.439A1.5 1.5 0 0 0 8.378 6H4.5Z"
									/>
								</svg>
							</button>
						</div>
					</div>
				{/each}
			</div>
		</div>
	{/if}
</div>
