<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { registerImage, deleteImage } from '$lib/api/images';
	import { toasts } from '$lib/stores/toasts.svelte';
	import { isApiError } from '$lib/api/errors';
	import PlatformLabel from '$lib/components/ui/PlatformLabel.svelte';
	import type { Platform, Arch } from '$lib/api/types';
	import type { PageData } from './$types';

	interface Props {
		data: PageData;
	}

	let { data }: Props = $props();

	let showRegister = $state(false);
	let name = $state('');
	let platform = $state<Platform>('windows');
	let arch = $state<Arch>('x64');
	let format = $state('qcow2');
	let description = $state('');
	let path = $state('');
	let busy = $state(false);
	let fieldErrors = $state<Record<string, string[]>>({});
	let searchQuery = $state('');

	const filtered = $derived(() => {
		const q = searchQuery.trim().toLowerCase();
		if (!q) return data.images;
		return data.images.filter(
			(img) =>
				img.name.toLowerCase().includes(q) ||
				img.platform.toLowerCase().includes(q) ||
				img.arch.toLowerCase().includes(q) ||
				img.format.toLowerCase().includes(q) ||
				img.description?.toLowerCase().includes(q)
		);
	});

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
			showRegister = false;
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
	<div class="flex items-center gap-3 text-sm text-[var(--color-text-secondary)]">
		<a href="/settings" class="hover:text-[var(--color-text-primary)]">Settings</a>
		<span>/</span>
		<span class="text-[var(--color-text-primary)]">Images</span>
	</div>

	<div class="flex items-center justify-between">
		<h1 class="text-3xl font-semibold text-[var(--color-text-primary)]">Images</h1>
		<button
			onclick={() => (showRegister = !showRegister)}
			class="rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white transition-colors hover:opacity-90"
		>
			{showRegister ? 'Cancel' : 'Register image'}
		</button>
	</div>

	{#if showRegister}
		<div
			class="space-y-4 rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-6"
		>
			<form onsubmit={handleRegister} class="grid grid-cols-1 gap-4 md:grid-cols-2">
				<div>
					<label for="img-name" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
						>Name</label
					>
					<input
						id="img-name"
						type="text"
						bind:value={name}
						required
						class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2 text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
					/>
					{#if fieldErrors.name}
						<span class="mt-1 block text-xs text-red-400">{fieldErrors.name.join(', ')}</span>
					{/if}
				</div>

				<div>
					<label for="img-path" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
						>Path (absolute)</label
					>
					<input
						id="img-path"
						type="text"
						bind:value={path}
						required
						placeholder="/var/lib/malbox/staging/image.qcow2"
						class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
					/>
					{#if fieldErrors.path}
						<span class="mt-1 block text-xs text-red-400">{fieldErrors.path.join(', ')}</span>
					{/if}
				</div>

				<div>
					<label for="img-platform" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
						>Platform</label
					>
					<select
						id="img-platform"
						bind:value={platform}
						class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2 text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
					>
						<option value="windows">windows</option>
						<option value="linux">linux</option>
					</select>
					{#if fieldErrors.platform}
						<span class="mt-1 block text-xs text-red-400">{fieldErrors.platform.join(', ')}</span>
					{/if}
				</div>

				<div>
					<label for="img-arch" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
						>Arch</label
					>
					<select
						id="img-arch"
						bind:value={arch}
						class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2 text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
					>
						<option value="x64">x64</option>
						<option value="x86">x86</option>
					</select>
					{#if fieldErrors.arch}
						<span class="mt-1 block text-xs text-red-400">{fieldErrors.arch.join(', ')}</span>
					{/if}
				</div>

				<div>
					<label for="img-format" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
						>Format</label
					>
					<input
						id="img-format"
						type="text"
						bind:value={format}
						class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2 text-sm text-[var(--color-text-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
					/>
				</div>

				<div>
					<label for="img-desc" class="mb-1 block text-xs text-[var(--color-text-secondary)]"
						>Description</label
					>
					<input
						id="img-desc"
						type="text"
						bind:value={description}
						placeholder="optional"
						class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-2 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
					/>
				</div>

				<div class="md:col-span-2">
					<button
						type="submit"
						class="rounded-lg bg-[var(--color-accent)] px-6 py-2 text-sm font-medium text-white transition-colors hover:bg-[var(--color-accent-hover)] disabled:opacity-50"
						disabled={busy}
					>
						{busy ? 'Registering...' : 'Register'}
					</button>
				</div>
			</form>
		</div>
	{/if}

	<div class="rounded-2xl bg-[var(--color-bg-secondary)] p-6 space-y-6">
		<div class="relative max-w-md">
			<input
				type="text"
				placeholder="Search images"
				bind:value={searchQuery}
				class="w-full rounded-lg bg-[var(--color-bg-tertiary)] px-4 py-3 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]/30"
			/>
		</div>

		{#if filtered().length === 0}
			<div class="py-12 text-center">
				<div
					class="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-xl bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)]"
				>
					<svg
						xmlns="http://www.w3.org/2000/svg"
						viewBox="0 0 20 20"
						fill="currentColor"
						class="size-6"
					>
						<path
							d="M10.362 1.093a.75.75 0 0 0-.724 0L2.523 5.018 10 9.143l7.477-4.125-7.115-3.925Z"
						/>
						<path d="M18 6.443l-7.25 4v8.25l6.862-3.786A.75.75 0 0 0 18 14.56V6.443Z" />
						<path d="M9.25 18.693v-8.25l-7.25-4v8.117a.75.75 0 0 0 .388.657l6.862 3.476Z" />
					</svg>
				</div>
				<p class="text-sm text-[var(--color-text-secondary)]">
					{data.images.length === 0
						? 'No images registered. Click "Register image" to add one.'
						: 'No images match your search.'}
				</p>
			</div>
		{:else}
			<div class="overflow-hidden rounded-lg border border-[var(--color-border)]">
				<div
					class="grid grid-cols-[1fr_100px_80px_80px_80px_80px] gap-4 border-b border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-4 py-2 text-xs font-medium text-[var(--color-text-secondary)]"
				>
					<div>Name</div>
					<div>Platform</div>
					<div>Arch</div>
					<div>Format</div>
					<div>Status</div>
					<div></div>
				</div>

				{#each filtered() as img (img.id)}
					<div
						class="grid grid-cols-[1fr_100px_80px_80px_80px_80px] items-center gap-4 border-b border-[var(--color-border)] px-4 py-3 text-sm last:border-b-0"
					>
						<div>
							<div class="text-[var(--color-text-primary)]">{img.name}</div>
							{#if img.description}
								<div
									class="truncate text-xs text-[var(--color-text-secondary)]"
									title={img.description}
								>
									{img.description}
								</div>
							{/if}
						</div>
						<div class="text-[var(--color-text-primary)]">
							<PlatformLabel platform={img.platform} />
						</div>
						<div class="text-[var(--color-text-secondary)]">{img.arch}</div>
						<div class="text-[var(--color-text-secondary)]">{img.format}</div>
						<div>
							{#if img.available}
								<span
									class="rounded bg-emerald-400/10 px-2 py-0.5 text-xs font-medium text-emerald-400"
								>
									ready
								</span>
							{:else}
								<span
									class="rounded bg-amber-400/10 px-2 py-0.5 text-xs font-medium text-amber-400"
								>
									pending
								</span>
							{/if}
						</div>
						<div>
							<button
								class="rounded-lg px-3 py-1 text-xs text-red-400 transition-colors hover:bg-red-400/10 hover:text-red-300"
								onclick={() => handleDelete(img.name)}
							>
								Delete
							</button>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>
