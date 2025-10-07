<script lang="ts">
	let statsTab = $state<'user' | 'globales'>('user');
</script>

<div class="bg-[var(--color-bg-secondary)] rounded-2xl p-6">
	<div class="flex items-center justify-between mb-6 pb-6 border-b border-[var(--color-border)]">
		<h2 class="text-xl font-semibold text-[var(--color-text-primary)]">Submission Statistics</h2>
		<div class="flex gap-2">
			<button
				onclick={() => (statsTab = 'user')}
				class="px-3 py-1 rounded-lg text-sm transition-colors {statsTab === 'user'
					? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:cursor-pointer'}"
			>
				User
			</button>
			<button
				onclick={() => (statsTab = 'globales')}
				class="px-3 py-1 rounded-lg text-sm transition-colors {statsTab === 'globales'
					? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:cursor-pointer'}"
			>
				Globales
			</button>
		</div>
	</div>

	<div class="grid grid-cols-2 gap-4">
		<div class="bg-[var(--color-bg-tertiary)] rounded-xl p-6 flex flex-col gap-2">
			<div class="text-[var(--color-text-secondary)] text-sm mb-2">Submission</div>
			<div class="text-5xl font-bold text-[var(--color-text-primary)]">1463</div>
		</div>
		<div class="bg-[var(--color-bg-tertiary)] rounded-xl p-6 flex flex-col gap-2">
			<div class="text-[var(--color-text-secondary)] text-sm mb-2">Verdicts</div>
			<div class="text-5xl font-bold text-[var(--color-text-primary)]">1356</div>
		</div>
		<div class="bg-[var(--color-bg-tertiary)] rounded-xl p-6 flex flex-col gap-2">
			<div class="text-[var(--color-text-secondary)] text-sm mb-2">Analysis in progress</div>
			<div class="text-5xl font-bold text-[var(--color-text-primary)]">14</div>
		</div>
		<div class="bg-[var(--color-bg-tertiary)] rounded-xl p-6 flex flex-col gap-2">
			<div class="text-[var(--color-text-secondary)] text-sm mb-2">Pending analysis</div>
			<div class="text-5xl font-bold text-[var(--color-text-primary)]">136</div>
		</div>
	</div>
</div>
