<script lang="ts">
	import { toasts, type ToastKind } from '$lib/stores/toasts.svelte';

	function tint(kind: ToastKind): string {
		switch (kind) {
			case 'success':
				return 'bg-green-500/20 border-green-500/40 text-green-200';
			case 'warning':
				return 'bg-yellow-500/20 border-yellow-500/40 text-yellow-200';
			case 'error':
				return 'bg-red-500/20 border-red-500/40 text-red-200';
			case 'info':
			default:
				return 'bg-[var(--color-bg-tertiary)] border-[var(--color-border)] text-[var(--color-text-primary)]';
		}
	}
</script>

<div class="fixed right-4 top-4 z-50 flex w-96 max-w-[calc(100vw-2rem)] flex-col gap-2">
	{#each toasts.list as toast (toast.id)}
		<div
			class="flex items-start gap-3 rounded-lg border p-4 shadow-lg {tint(toast.kind)}"
			role="status"
		>
			<span class="flex-1 text-sm leading-snug">{toast.message}</span>
			<button
				class="text-sm opacity-60 hover:opacity-100"
				onclick={() => toasts.dismiss(toast.id)}
				aria-label="Dismiss"
			>
				✕
			</button>
		</div>
	{/each}
</div>
