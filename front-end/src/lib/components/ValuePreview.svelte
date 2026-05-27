<script lang="ts">
	import { openSearchPanel } from '@codemirror/search';
	import { valuePreviewStore } from '$lib/stores/valuePreview.svelte';
	import CodeMirrorEditor from '$lib/components/CodeMirrorEditor.svelte';
	import CopyButton from '$lib/components/ui/CopyButton.svelte';
	import ContextMenu from '$lib/components/context-menu/ContextMenu.svelte';
	import { contextmenu } from '$lib/actions/contextmenu';
	import { HashIcon, WrapTextIcon, SearchIcon } from '@lucide/svelte';

	let dialogEl: HTMLDialogElement | undefined = $state();
	let editorRef: ReturnType<typeof CodeMirrorEditor> | undefined = $state();
	let showLineNumbers = $state(true);
	let wordWrap = $state(false);

	$effect(() => {
		if (valuePreviewStore.open && dialogEl && !dialogEl.open) {
			dialogEl.showModal();
		}
	});

	function handleDialogClose() {
		valuePreviewStore.close();
		showLineNumbers = true;
		wordWrap = false;
	}

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === dialogEl) {
			dialogEl?.close();
		}
	}

	function handleSearch() {
		const view = editorRef?.getView();
		if (view) {
			view.focus();
			openSearchPanel(view);
		}
	}
</script>

<dialog
	bind:this={dialogEl}
	onclose={handleDialogClose}
	onclick={handleBackdropClick}
	class="value-preview m-auto max-h-[90vh] w-[90vw] max-w-3xl rounded-2xl border border-[var(--color-border)] bg-[var(--color-bg-primary)] p-0"
>
	{#if valuePreviewStore.open}
		<div class="flex items-center gap-3 bg-[var(--color-bg-tertiary)] px-5 py-3">
			<div class="min-w-0 flex-1">
				<div class="font-medium text-[var(--color-text-primary)]">
					{valuePreviewStore.label}
				</div>
				<div class="text-xs text-[var(--color-text-secondary)]">
					{valuePreviewStore.source}
				</div>
			</div>
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

		<div
			class="flex items-center justify-end gap-1.5 border-b border-[var(--color-border)]/30 bg-[var(--color-bg-primary)] px-4 py-1.5"
		>
			<button
				type="button"
				class="rounded p-1 transition-colors {showLineNumbers
					? 'text-[var(--color-accent)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				onclick={() => (showLineNumbers = !showLineNumbers)}
				title="Toggle line numbers"
			>
				<HashIcon class="size-3.5" />
			</button>
			<button
				type="button"
				class="rounded p-1 transition-colors {wordWrap
					? 'text-[var(--color-accent)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				onclick={() => (wordWrap = !wordWrap)}
				title="Toggle word wrap"
			>
				<WrapTextIcon class="size-3.5" />
			</button>
			<button
				type="button"
				class="rounded p-1 text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text-primary)]"
				onclick={handleSearch}
				title="Find in code"
			>
				<SearchIcon class="size-3.5" />
			</button>
			<CopyButton value={valuePreviewStore.value} size="sm" />
		</div>

		<div use:contextmenu={{ type: 'code-block', value: '', selectionOnly: true }}>
			<CodeMirrorEditor
				bind:this={editorRef}
				value={valuePreviewStore.value}
				readonly
				{showLineNumbers}
				lineWrapping={wordWrap}
				maxHeight="calc(90vh - 7.5rem)"
			/>
		</div>

		<ContextMenu />
	{/if}
</dialog>

<style>
	dialog::backdrop {
		background: rgba(0, 0, 0, 0.6);
	}

	.value-preview :global(.cm-host) {
		border-radius: 0 0 1rem 1rem;
	}
</style>
