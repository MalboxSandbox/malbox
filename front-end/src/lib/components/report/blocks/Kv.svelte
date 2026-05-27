<script lang="ts">
	import type { KvPair } from '$lib/api/types';
	import { contextmenu } from '$lib/actions/contextmenu';
	import { truncateWithTail } from '$lib/utils/truncate';

	interface Props {
		pairs: KvPair[];
	}
	let { pairs }: Props = $props();

	let copiedId = $state<string | null>(null);

	async function copy(text: string, id: string) {
		await navigator.clipboard.writeText(text);
		copiedId = id;
		setTimeout(() => (copiedId = null), 1500);
	}
</script>

<dl class="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-2 text-sm">
	{#each pairs as p, i (i)}
		{@const parts = truncateWithTail(p.value, 100, 12)}
		<dt class="max-w-56">
			<button
				type="button"
				class="w-full cursor-copy truncate text-left transition-colors {copiedId === `k${i}`
					? 'text-[var(--color-accent)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
				onclick={() => copy(p.key, `k${i}`)}
				title={p.key}
			>
				{p.key}
			</button>
		</dt>
		<dd>
			<button
				type="button"
				class="w-full cursor-copy text-left transition-colors {p.mono
					? 'font-mono text-xs'
					: ''} {copiedId === `v${i}`
					? 'text-[var(--color-accent)]'
					: 'text-[var(--color-text-primary)] hover:text-[var(--color-accent)]'}"
				onclick={() => copy(p.value, `v${i}`)}
				title="Click to copy"
				use:contextmenu={{ type: 'generic', value: p.value, metadata: { label: p.key } }}
			>
				<span class="break-all"
					>{parts.head}{#if parts.truncated}<span class="text-[var(--color-text-secondary)]">…</span
						>{parts.tail}{/if}</span
				>
			</button>
		</dd>
	{/each}
</dl>
