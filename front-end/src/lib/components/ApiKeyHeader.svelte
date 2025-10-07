<script lang="ts">
	interface Props {
		selectedKey: string | null;
		onManageKeys: () => void;
	}

	let { selectedKey, onManageKeys }: Props = $props();
</script>

<div class="flex items-center justify-between mb-8">
	<div class="flex items-center gap-3">
		<span class="text-[var(--color-text-secondary)] text-sm">
			{selectedKey || 'No keys selected'}
		</span>
		<button
			class="w-6 h-6 rounded flex items-center justify-center bg-[var(--color-bg-tertiary)] hover:bg-[var(--color-bg-card)] transition-colors"
			aria-label="Add API key"
		>
			<span class="text-[var(--color-text-secondary)] text-lg">+</span>
		</button>
	</div>
	<button
		onclick={onManageKeys}
		class="px-6 py-2.5 bg-[var(--color-accent)] hover:bg-[var(--color-accent-hover)] text-[var(--color-text-primary)] rounded-lg font-medium transition-colors text-sm"
	>
		Manage API Keys
	</button>
</div>
