<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { registerImage, deleteImage } from '$lib/api/images';
	import { toasts } from '$lib/stores/toasts.svelte';
	import { isApiError } from '$lib/api/errors';
	import type { PageData } from './$types';
	import PlatformLabel from '$lib/components/ui/PlatformLabel.svelte';
	import type { Platform, Arch } from '$lib/api/types';

	interface Props {
		data: PageData;
	}

	let { data }: Props = $props();

	let name = $state('');
	let platform = $state<Platform>('windows');
	let arch = $state<Arch>('x64');
	let format = $state('qcow2');
	let description = $state('');
	let path = $state('');
	let busy = $state(false);
	let fieldErrors = $state<Record<string, string[]>>({});

	async function handleRegister(e: SubmitEvent) {
		e.preventDefault();
		if (busy) return;
		busy = true;
		fieldErrors = {};
		try {
			await registerImage(fetch, {
				name,
				platform,
				arch,
				format: format || undefined,
				description: description || undefined,
				path
			});
			toasts.push({ kind: 'success', message: `Image "${name}" registered.` });
			name = '';
			description = '';
			path = '';
			await invalidate('malbox:images');
		} catch (err) {
			if (isApiError(err) && err.kind === 'validation' && err.fieldErrors) {
				fieldErrors = err.fieldErrors;
			} else {
				const msg = isApiError(err) ? err.message : 'Register failed.';
				toasts.push({ kind: 'error', message: msg });
			}
		} finally {
			busy = false;
		}
	}

	async function handleDelete(imgName: string) {
		if (!confirm(`Delete image "${imgName}"?`)) return;
		try {
			await deleteImage(fetch, imgName);
			toasts.push({ kind: 'success', message: `Image "${imgName}" deleted.` });
			await invalidate('malbox:images');
		} catch (err) {
			const msg = isApiError(err) ? err.message : 'Delete failed.';
			toasts.push({ kind: 'error', message: msg });
		}
	}
</script>

<div class="mx-auto max-w-7xl space-y-6">
	<h1 class="text-3xl font-semibold text-[var(--color-text-primary)]">Images</h1>

	<div class="space-y-4 rounded-2xl bg-[var(--color-bg-secondary)] p-8">
		<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">Register image</h2>
		<form onsubmit={handleRegister} class="grid grid-cols-1 gap-4 md:grid-cols-2">
			<label class="flex flex-col gap-1">
				<span class="text-xs text-[var(--color-text-secondary)]">Name</span>
				<input
					type="text"
					bind:value={name}
					required
					class="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm text-[var(--color-text-primary)]"
				/>
				{#if fieldErrors.name}
					<span class="text-xs text-red-400">{fieldErrors.name.join(', ')}</span>
				{/if}
			</label>

			<label class="flex flex-col gap-1">
				<span class="text-xs text-[var(--color-text-secondary)]">Path (absolute)</span>
				<input
					type="text"
					bind:value={path}
					required
					placeholder="/var/lib/malbox/staging/image.qcow2"
					class="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm text-[var(--color-text-primary)]"
				/>
				{#if fieldErrors.path}
					<span class="text-xs text-red-400">{fieldErrors.path.join(', ')}</span>
				{/if}
			</label>

			<label class="flex flex-col gap-1">
				<span class="text-xs text-[var(--color-text-secondary)]">Platform</span>
				<select
					bind:value={platform}
					class="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm text-[var(--color-text-primary)]"
				>
					<option value="windows">windows</option>
					<option value="linux">linux</option>
				</select>
				{#if fieldErrors.platform}
					<span class="text-xs text-red-400">{fieldErrors.platform.join(', ')}</span>
				{/if}
			</label>

			<label class="flex flex-col gap-1">
				<span class="text-xs text-[var(--color-text-secondary)]">Arch</span>
				<select
					bind:value={arch}
					class="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm text-[var(--color-text-primary)]"
				>
					<option value="x64">x64</option>
					<option value="x86">x86</option>
				</select>
				{#if fieldErrors.arch}
					<span class="text-xs text-red-400">{fieldErrors.arch.join(', ')}</span>
				{/if}
			</label>

			<label class="flex flex-col gap-1">
				<span class="text-xs text-[var(--color-text-secondary)]">Format</span>
				<input
					type="text"
					bind:value={format}
					class="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm text-[var(--color-text-primary)]"
				/>
			</label>

			<label class="flex flex-col gap-1 md:col-span-2">
				<span class="text-xs text-[var(--color-text-secondary)]">Description (optional)</span>
				<input
					type="text"
					bind:value={description}
					class="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-3 py-2 text-sm text-[var(--color-text-primary)]"
				/>
			</label>

			<div class="md:col-span-2">
				<button
					type="submit"
					class="rounded-lg bg-[var(--color-accent)] px-6 py-2 text-sm font-medium text-white transition-colors hover:bg-[var(--color-accent-hover)] disabled:opacity-50"
					disabled={busy}
				>
					{busy ? 'Registering…' : 'Register'}
				</button>
			</div>
		</form>
	</div>

	<div class="overflow-hidden rounded-2xl bg-[var(--color-bg-secondary)]">
		<div
			class="grid grid-cols-[1fr_120px_120px_120px_100px] gap-4 border-b border-[var(--color-border)] px-6 py-4 text-sm font-medium text-[var(--color-text-secondary)]"
		>
			<div>Name</div>
			<div>Platform</div>
			<div>Arch</div>
			<div>Format</div>
			<div>Action</div>
		</div>
		{#if data.images.length === 0}
			<div class="p-8 text-center text-sm text-[var(--color-text-secondary)]">
				No images registered.
			</div>
		{:else}
			{#each data.images as img (img.id)}
				<div
					class="grid grid-cols-[1fr_120px_120px_120px_100px] items-center gap-4 border-b border-[var(--color-border)] px-6 py-4 text-sm last:border-b-0"
				>
					<div class="text-[var(--color-text-primary)]">{img.name}</div>
					<div class="text-[var(--color-text-primary)]">
						<PlatformLabel platform={img.platform} />
					</div>
					<div class="text-[var(--color-text-primary)]">{img.arch}</div>
					<div class="text-[var(--color-text-primary)]">{img.format}</div>
					<div>
						<button
							class="rounded-lg px-3 py-1 text-xs text-red-400 hover:bg-[var(--color-bg-tertiary)] hover:text-red-300"
							onclick={() => handleDelete(img.name)}
						>
							Delete
						</button>
					</div>
				</div>
			{/each}
		{/if}
	</div>
</div>
