<script lang="ts">
	import type { SubmissionItem } from '$lib/types/submission';

	interface Props {
		submissions: SubmissionItem[];
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

<div class="bg-[var(--color-bg-secondary)] rounded-xl overflow-hidden">
	<!-- Table Header -->
	<div
		class="grid grid-cols-[180px_1fr_120px_180px_180px_150px] gap-4 px-8 py-4 border-b border-[var(--color-border)]"
	>
		<div class="text-[var(--color-text-secondary)] text-sm font-medium">Date</div>
		<div class="text-[var(--color-text-secondary)] text-sm font-medium">Submission</div>
		<div class="text-[var(--color-text-secondary)] text-sm font-medium">Status</div>
		<div class="text-[var(--color-text-secondary)] text-sm font-medium">Note</div>
		<div class="text-[var(--color-text-secondary)] text-sm font-medium">API Key</div>
		<div class="text-[var(--color-text-secondary)] text-sm font-medium">Action</div>
	</div>

	<!-- Table Body -->
	<div>
		{#each submissions as submission}
			<div
				class="grid grid-cols-[180px_1fr_120px_180px_180px_150px] gap-4 px-8 py-6 border-b border-[var(--color-border)] last:border-b-0 hover:bg-[var(--color-bg-tertiary)] transition-colors"
			>
				<!-- Date -->
				<div class="flex flex-col gap-1">
					<span class="text-[var(--color-text-primary)] text-sm font-medium">
						{submission.date}
					</span>
					<span class="text-[var(--color-text-secondary)] text-sm">{submission.time}</span>
				</div>

				<!-- Submission -->
				<div class="flex items-center gap-3 min-w-0">
					<div
						class="w-8 h-8 rounded-lg bg-[var(--color-bg-tertiary)] flex items-center justify-center text-[var(--color-text-secondary)] flex-shrink-0"
					>
						{@html getTypeIcon(submission.type)}
					</div>
					<span class="text-[var(--color-text-primary)] text-sm truncate min-w-0 block">
						{submission.value}
					</span>
				</div>

				<!-- Status -->
				<div class="flex items-center">
					{#if submission.status === 'finished'}
						<span
							class="px-3 py-1 bg-[var(--color-accent)]/20 text-[var(--color-accent)] text-xs rounded font-medium whitespace-nowrap"
						>
							Finished
						</span>
					{:else}
						<span
							class="px-3 py-1 bg-[var(--color-text-secondary)]/20 text-[var(--color-text-secondary)] text-xs rounded font-medium whitespace-nowrap"
						>
							Pending
						</span>
					{/if}
				</div>

				<!-- Note (Progress) -->
				<div class="flex items-center gap-3">
					<span class="text-[var(--color-text-primary)] text-sm min-w-[45px] flex-shrink-0">
						{submission.status === 'finished' ? submission.progress : '-'}/{submission.total}
					</span>
					<div class="flex-1 h-1.5 bg-[var(--color-border)] rounded-full overflow-hidden min-w-0">
						{#if submission.status === 'finished'}
							<div
								class="h-full bg-[var(--color-accent)] rounded-full transition-all"
								style="width: {(submission.progress / submission.total) * 100}%"
							></div>
						{/if}
					</div>
				</div>

				<!-- API Key -->
				<div class="flex items-center gap-2">
					{#if submission.apiKey}
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
					{:else}
						<span class="text-[var(--color-text-secondary)] text-sm">No key</span>
					{/if}
				</div>

				<!-- Action -->
				<div>
					<button
						class="px-4 py-2 bg-[var(--color-bg-card)] hover:bg-[var(--color-bg-tertiary)] text-[var(--color-text-primary)] text-sm rounded-lg transition-colors flex items-center gap-2"
					>
						See the result
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
