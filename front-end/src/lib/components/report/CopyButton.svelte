<script lang="ts">
	interface Props {
		text: string;
	}

	let { text }: Props = $props();

	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	async function copy() {
		await navigator.clipboard.writeText(text);
		copied = true;
		clearTimeout(timer);
		timer = setTimeout(() => (copied = false), 1600);
	}
</script>

<button
	type="button"
	class="copy-btn"
	class:copied
	onclick={copy}
	aria-label={copied ? 'Copied' : 'Copy to clipboard'}
	title={copied ? 'Copied!' : 'Copy to clipboard'}
>
	<svg
		class="icon"
		xmlns="http://www.w3.org/2000/svg"
		viewBox="0 0 20 20"
		fill="currentColor"
		aria-hidden="true"
	>
		<path
			d="M7 3.5A1.5 1.5 0 0 1 8.5 2h3.879a1.5 1.5 0 0 1 1.06.44l3.122 3.12A1.5 1.5 0 0 1 17 6.622V12.5a1.5 1.5 0 0 1-1.5 1.5h-1v-3.379a3 3 0 0 0-.879-2.121L10.5 5.379A3 3 0 0 0 8.379 4.5H7v-1Z"
		/>
		<path
			d="M4.5 6A1.5 1.5 0 0 0 3 7.5v9A1.5 1.5 0 0 0 4.5 18h7a1.5 1.5 0 0 0 1.5-1.5v-5.879a1.5 1.5 0 0 0-.44-1.06L9.44 6.439A1.5 1.5 0 0 0 8.378 6H4.5Z"
		/>
	</svg>
</button>

<style>
	.copy-btn {
		display: grid;
		place-items: center;
		width: 20px;
		height: 20px;
		flex-shrink: 0;
		padding: 0;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		/* Smoothly tints in both directions: secondary -> accent on copy, and back. */
		transition: color 0.25s ease;
	}

	.copy-btn:hover {
		color: var(--color-text-primary);
	}

	.copy-btn.copied {
		color: var(--color-accent);
	}

	.icon {
		width: 15px;
		height: 15px;
	}

	/* A gentle squash-and-settle pop confirms the copy without changing the icon. */
	.copy-btn.copied .icon {
		animation: pop 0.4s ease-out;
	}

	@keyframes pop {
		0% {
			transform: scale(1);
		}
		35% {
			transform: scale(0.88);
		}
		70% {
			transform: scale(1.08);
		}
		100% {
			transform: scale(1);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.copy-btn {
			transition: color 0.15s ease;
		}
		.copy-btn.copied .icon {
			animation: none;
		}
	}
</style>
