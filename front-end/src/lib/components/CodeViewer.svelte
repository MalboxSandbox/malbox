<script lang="ts">
	import { openSearchPanel } from '@codemirror/search';
	import CodeMirrorEditor from '$lib/components/CodeMirrorEditor.svelte';
	import CopyButton from '$lib/components/ui/CopyButton.svelte';
	import { HashIcon, WrapTextIcon, SearchIcon } from '@lucide/svelte';

	interface Props {
		code: string;
		language?: string;
		maxHeight?: string;
	}
	let { code, language = '', maxHeight = '400px' }: Props = $props();

	let showLineNumbers = $state(true);
	let wordWrap = $state(false);
	let editorRef: ReturnType<typeof CodeMirrorEditor> | undefined = $state();

	function handleSearch() {
		const view = editorRef?.getView();
		if (view) {
			view.focus();
			openSearchPanel(view);
		}
	}
</script>

<div class="overflow-hidden rounded-lg bg-[var(--color-bg-primary)]">
	<!-- Toolbar -->
	<div class="flex items-center justify-between px-4 py-2">
		{#if language}
			<span class="font-mono text-[10px] uppercase text-[var(--color-text-secondary)]">
				{language}
			</span>
		{:else}
			<span></span>
		{/if}
		<div class="flex items-center gap-1.5">
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
			<CopyButton value={code} size="sm" />
		</div>
	</div>

	<!-- Editor -->
	<div class="border-t border-[var(--color-border)]/30">
		<CodeMirrorEditor
			bind:this={editorRef}
			value={code}
			{language}
			readonly
			{showLineNumbers}
			lineWrapping={wordWrap}
			maxHeight={maxHeight === 'none' ? undefined : maxHeight}
		/>
	</div>
</div>
