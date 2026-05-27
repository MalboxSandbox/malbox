<script lang="ts">
	import { formatBytes } from '$lib/api/format';
	import { previewKind, previewLanguage, type PreviewKind } from './artifact';
	import CopyButton from '$lib/components/ui/CopyButton.svelte';
	import CodeViewer from '$lib/components/CodeViewer.svelte';
	import type { ArtifactLink } from '$lib/api/types';

	const MAX_TEXT_BYTES = 5 * 1024 * 1024;

	interface Props {
		artifact: ArtifactLink | null;
		onclose: () => void;
	}
	let { artifact, onclose }: Props = $props();

	let dialogEl: HTMLDialogElement | undefined = $state();
	let textContent = $state<string | null>(null);
	let loading = $state(false);
	let error = $state<string | null>(null);

	const kind = $derived<PreviewKind>(
		artifact ? previewKind(artifact.result_name, artifact.format) : 'none'
	);
	const language = $derived(artifact ? previewLanguage(artifact.result_name) : '');

	function fileName(path: string): string {
		const sep = path.lastIndexOf('/');
		const bsep = path.lastIndexOf('\\');
		const idx = Math.max(sep, bsep);
		return idx >= 0 ? path.slice(idx + 1) : path;
	}

	function pathSegments(path: string): string[] {
		return path.split(/[/\\]/).filter(Boolean);
	}

	const segments = $derived(artifact ? pathSegments(artifact.result_name) : []);

	let copiedPath = $state(false);

	async function copyPath(path: string) {
		await navigator.clipboard.writeText(path);
		copiedPath = true;
		setTimeout(() => (copiedPath = false), 1500);
	}

	$effect(() => {
		if (artifact && dialogEl && !dialogEl.open) {
			dialogEl.showModal();
		}
	});

	$effect(() => {
		if (!artifact) return;
		const k = kind;
		if (k !== 'text' && k !== 'code' && k !== 'json') return;

		if (artifact.size_bytes > MAX_TEXT_BYTES) {
			error = `File too large to preview (${formatBytes(artifact.size_bytes)})`;
			return;
		}

		const controller = new AbortController();
		loading = true;
		error = null;
		textContent = null;

		fetch(artifact.url, { signal: controller.signal })
			.then((r) => {
				if (!r.ok) throw new Error(`HTTP ${r.status}`);
				return r.text();
			})
			.then((text) => {
				textContent = text;
			})
			.catch((e) => {
				if (e.name !== 'AbortError') error = e instanceof Error ? e.message : 'Failed to load';
			})
			.finally(() => {
				loading = false;
			});

		return () => controller.abort();
	});

	function handleDialogClose() {
		textContent = null;
		loading = false;
		error = null;
		onclose();
	}

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === dialogEl) {
			dialogEl?.close();
		}
	}

	const prettyJson = $derived.by(() => {
		if (kind !== 'json' || !textContent) return '';
		try {
			return JSON.stringify(JSON.parse(textContent), null, 2);
		} catch {
			return textContent;
		}
	});
</script>

<dialog
	bind:this={dialogEl}
	onclose={handleDialogClose}
	onclick={handleBackdropClick}
	class="m-auto max-h-[90vh] w-[90vw] max-w-5xl rounded-2xl border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] p-0"
>
	{#if artifact}
		{@const name = fileName(artifact.result_name)}
		<div class="flex items-center gap-3 border-b border-[var(--color-border)]/50 px-5 py-3">
			<div class="min-w-0 flex-1">
				<button
					type="button"
					class="flex min-w-0 items-center gap-0 truncate text-sm"
					title={copiedPath ? 'Copied!' : `Click to copy: ${artifact.result_name}`}
					onclick={() => copyPath(artifact.result_name)}
				>
					{#each segments as seg, i (i)}
						{#if i > 0}
							<span class="mx-1 text-[var(--color-text-secondary)]/50">&#8250;</span>
						{/if}
						{#if i === segments.length - 1}
							<span class="font-medium text-[var(--color-text-primary)]">{seg}</span>
						{:else}
							<span class="text-[var(--color-text-secondary)]">{seg}</span>
						{/if}
					{/each}
				</button>
				<div class="mt-0.5 flex items-center gap-2 text-xs text-[var(--color-text-secondary)]">
					<span class="uppercase">{artifact.format}</span>
					<span>&middot;</span>
					<span>{formatBytes(artifact.size_bytes)}</span>
				</div>
			</div>
			<a
				href={artifact.url}
				download={artifact.result_name}
				class="inline-flex items-center gap-1.5 rounded-lg bg-[var(--color-bg-card)] px-3 py-1.5 text-xs text-[var(--color-text-primary)] transition-colors hover:bg-[var(--color-bg-primary)]"
			>
				<svg
					xmlns="http://www.w3.org/2000/svg"
					viewBox="0 0 20 20"
					fill="currentColor"
					class="size-3.5"
				>
					<path
						d="M10.75 2.75a.75.75 0 0 0-1.5 0v8.614L6.295 8.235a.75.75 0 1 0-1.09 1.03l4.25 4.5a.75.75 0 0 0 1.09 0l4.25-4.5a.75.75 0 0 0-1.09-1.03l-2.955 3.129V2.75Z"
					/>
					<path
						d="M3.5 12.75a.75.75 0 0 0-1.5 0v2.5A2.75 2.75 0 0 0 4.75 18h10.5A2.75 2.75 0 0 0 18 15.25v-2.5a.75.75 0 0 0-1.5 0v2.5c0 .69-.56 1.25-1.25 1.25H4.75c-.69 0-1.25-.56-1.25-1.25v-2.5Z"
					/>
				</svg>
				Download
			</a>
			<button
				type="button"
				onclick={() => dialogEl?.close()}
				aria-label="Close preview"
				class="rounded-lg p-1.5 text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-card)] hover:text-[var(--color-text-primary)]"
			>
				<svg class="size-5" viewBox="0 0 20 20" fill="currentColor">
					<path
						d="M6.28 5.22a.75.75 0 00-1.06 1.06L8.94 10l-3.72 3.72a.75.75 0 101.06 1.06L10 11.06l3.72 3.72a.75.75 0 101.06-1.06L11.06 10l3.72-3.72a.75.75 0 00-1.06-1.06L10 8.94 6.28 5.22z"
					/>
				</svg>
			</button>
		</div>

		<div class="overflow-auto p-5" style="max-height: calc(90vh - 4rem);">
			{#if kind === 'image'}
				<div class="flex items-center justify-center">
					<img
						src={artifact.url}
						alt={name}
						class="max-h-[75vh] max-w-full rounded-lg object-contain"
					/>
				</div>
			{:else if kind === 'pdf'}
				<object
					data={artifact.url}
					type="application/pdf"
					title="PDF preview: {name}"
					class="h-[75vh] w-full rounded-lg"
				>
					<p class="text-sm text-[var(--color-text-secondary)]">
						PDF preview not supported in this browser.
						<a
							href={artifact.url}
							download={artifact.result_name}
							class="text-[var(--color-accent)] underline">Download instead</a
						>
					</p>
				</object>
			{:else if loading}
				<div class="flex items-center justify-center py-12">
					<div
						class="size-6 animate-spin rounded-full border-2 border-[var(--color-text-secondary)] border-t-[var(--color-accent)]"
					></div>
				</div>
			{:else if error}
				<div
					class="rounded-lg border-l-2 border-amber-500 bg-amber-500/10 p-4 text-sm text-amber-200"
				>
					{error}
				</div>
			{:else if kind === 'json'}
				{#if textContent}
					<CodeViewer code={prettyJson} language="json" maxHeight="none" />
				{/if}
			{:else if kind === 'code'}
				{#if textContent}
					<CodeViewer code={textContent} {language} maxHeight="none" />
				{/if}
			{:else if kind === 'text'}
				<div class="relative">
					{#if textContent}
						<div class="absolute right-2 top-2">
							<CopyButton value={textContent} size="sm" />
						</div>
					{/if}
					<pre
						class="overflow-x-auto whitespace-pre-wrap rounded-lg bg-[var(--color-bg-primary)] p-4 font-mono text-xs leading-relaxed text-[var(--color-text-primary)]">{textContent}</pre>
				</div>
			{:else}
				<div
					class="flex flex-col items-center justify-center gap-3 py-12 text-[var(--color-text-secondary)]"
				>
					<svg
						xmlns="http://www.w3.org/2000/svg"
						viewBox="0 0 20 20"
						fill="currentColor"
						class="size-8"
					>
						<path
							fill-rule="evenodd"
							d="M4.5 2A1.5 1.5 0 0 0 3 3.5v13A1.5 1.5 0 0 0 4.5 18h11a1.5 1.5 0 0 0 1.5-1.5V7.621a1.5 1.5 0 0 0-.44-1.06l-4.12-4.122A1.5 1.5 0 0 0 11.378 2H4.5Z"
							clip-rule="evenodd"
						/>
					</svg>
					<p class="text-sm">Preview not available for this file type</p>
				</div>
			{/if}
		</div>
	{/if}
</dialog>

<style>
	dialog::backdrop {
		background: rgba(0, 0, 0, 0.6);
	}
</style>
