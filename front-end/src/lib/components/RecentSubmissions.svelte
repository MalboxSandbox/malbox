<script lang="ts">
	import type { Submission } from '$lib/types/automation';

	interface Props {
		submissions: Submission[];
	}

	let { submissions }: Props = $props();

	function getTypeIcon(type: 'url' | 'file' | 'hash') {
		switch (type) {
			case 'url':
				return `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />
					<path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
				</svg>`;
			case 'file':
				return `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z" />
					<polyline points="13 2 13 9 20 9" />
				</svg>`;
			case 'hash':
				return `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<line x1="4" y1="9" x2="20" y2="9" />
					<line x1="4" y1="15" x2="20" y2="15" />
					<line x1="10" y1="3" x2="8" y2="21" />
					<line x1="16" y1="3" x2="14" y2="21" />
				</svg>`;
		}
	}
</script>

<div class="bg-[var(--color-bg-secondary)] rounded-xl p-6">
	<h2 class="text-[var(--color-text-primary)] text-lg font-semibold mb-6">Recent Submissions</h2>

	<div class="space-y-4">
		{#each submissions as submission}
			<div
				class="flex items-center gap-6 py-4 border-b border-[var(--color-border)] last:border-b-0"
			>
				<!-- Date and Time -->
				<div class="flex items-baseline gap-2 min-w-[140px]">
					<span class="text-[var(--color-text-primary)] text-sm">{submission.date}</span>
					<span class="text-[var(--color-text-secondary)] text-sm">{submission.time}</span>
				</div>

				<!-- Submission Value with Type Icon -->
				<div class="flex items-center gap-3 flex-1">
					<div
						class="w-8 h-8 rounded-lg bg-[var(--color-bg-tertiary)] flex items-center justify-center text-[var(--color-text-secondary)]"
					>
						{@html getTypeIcon(submission.type)}
					</div>
					<span class="text-[var(--color-text-primary)] text-sm truncate flex-1">
						{submission.value}
					</span>
					{#if submission.status === 'finished'}
						<span
							class="px-3 py-1 bg-[var(--color-accent)]/20 text-[var(--color-accent)] text-xs rounded-full whitespace-nowrap"
						>
							Finished
						</span>
					{:else}
						<span
							class="px-3 py-1 bg-[var(--color-text-secondary)]/20 text-[var(--color-text-secondary)] text-xs rounded-full whitespace-nowrap"
						>
							Pending
						</span>
					{/if}
				</div>

				<!-- Progress -->
				<div class="flex items-center gap-3 min-w-[140px]">
					{#if submission.status === 'finished'}
						<span class="text-[var(--color-text-primary)] text-sm">
							{submission.progress}/{submission.total}
						</span>
						<div class="flex-1 h-1.5 bg-[var(--color-border)] rounded-full overflow-hidden">
							<div
								class="h-full bg-[var(--color-accent)] rounded-full transition-all"
								style="width: {(submission.progress / submission.total) * 100}%"
							></div>
						</div>
					{:else}
						<span class="text-[var(--color-text-primary)] text-sm">-/{submission.total}</span>
						<div class="flex-1 h-1.5 bg-[var(--color-border)] rounded-full"></div>
					{/if}
				</div>

				<!-- API Key -->
				<div class="flex items-center gap-2 min-w-[160px]">
					<span class="text-[var(--color-text-primary)] text-sm">{submission.apiKey}</span>
					<button
						class="text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] transition-colors"
					>
						<svg
							width="16"
							height="16"
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							stroke-width="2"
						>
							<path d="M7 17L17 7M17 7H7M17 7V17" />
						</svg>
					</button>
				</div>
			</div>
		{/each}
	</div>
</div>
