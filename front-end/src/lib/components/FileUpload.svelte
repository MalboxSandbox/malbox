<script lang="ts">
	import { fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { invalidate, goto } from '$app/navigation';
	import { toasts } from '$lib/stores/toasts.svelte';
	import {
		createTaskFromFile,
		createTaskFromUrl,
		lookupSample,
		rescanSample
	} from '$lib/api/tasks';
	import { isApiError } from '$lib/api/errors';
	import { formatBytes } from '$lib/api/format';
	import SubmissionConfigModal from '$lib/components/SubmissionConfigModal.svelte';
	import type { Platform, SampleLookup } from '$lib/api/types';

	let activeTab = $state<'file' | 'url'>('file');
	let urlInput = $state('');
	let fileInput: HTMLInputElement | undefined = $state();
	let selectedFile = $state<File | null>(null);
	let submitting = $state(false);
	let dragOver = $state(false);

	let hashLookup = $state<SampleLookup | null>(null);
	let lookingUp = $state(false);
	let lookupNotFound = $state(false);

	let submissionConfig = $state<{
		timeout: number | null;
		platform: Platform | null;
		tags: string[];
		plugins: string[];
		machineId: number | null;
		snapshotId: string | null;
		priority: number;
		vmMode: 'windows' | 'linux' | 'no-vm';
	}>({
		timeout: null,
		platform: null,
		tags: [],
		plugins: [],
		machineId: null,
		snapshotId: null,
		priority: 2,
		vmMode: 'windows'
	});

	function defaultConfig() {
		return {
			timeout: null,
			platform: null,
			tags: [],
			plugins: [],
			machineId: null,
			snapshotId: null,
			priority: 2,
			vmMode: 'windows' as const
		};
	}

	function onFileChosen(file: File | null) {
		selectedFile = file;
	}

	function onDrop(e: DragEvent) {
		e.preventDefault();
		dragOver = false;
		if (e.dataTransfer?.files?.[0]) onFileChosen(e.dataTransfer.files[0]);
	}

	function buildOptionalFields() {
		return {
			...(submissionConfig.timeout !== null && { timeout: submissionConfig.timeout }),
			...(submissionConfig.tags.length > 0 && { tags: submissionConfig.tags.join(',') }),
			...(submissionConfig.plugins.length > 0 && {
				plugins: submissionConfig.plugins.join(',')
			}),
			...(submissionConfig.snapshotId !== null && {
				snapshot_id: submissionConfig.snapshotId
			}),
			priority: submissionConfig.priority
		};
	}

	async function submitFile() {
		if (!selectedFile || submitting) return;
		submitting = true;
		try {
			const { task_id } = await createTaskFromFile(fetch, {
				file: selectedFile,
				...(submissionConfig.timeout !== null && { timeout: submissionConfig.timeout }),
				...(submissionConfig.vmMode !== 'no-vm' && {
					platform: submissionConfig.vmMode as Platform
				}),
				...(submissionConfig.tags.length > 0 && { tags: submissionConfig.tags.join(',') }),
				...(submissionConfig.plugins.length > 0 && {
					plugins: submissionConfig.plugins.join(',')
				}),
				...(submissionConfig.snapshotId !== null && {
					snapshot_id: submissionConfig.snapshotId
				}),
				priority: submissionConfig.priority
			});
			toasts.push({ kind: 'success', message: `Task #${task_id} submitted.` });
			await invalidate('malbox:tasks');
			selectedFile = null;
			if (fileInput) fileInput.value = '';
			submissionConfig = defaultConfig();
			goto(`/submissions/${task_id}`);
		} catch (err) {
			const msg = isApiError(err) ? err.message : 'Upload failed.';
			toasts.push({ kind: 'error', message: msg });
		} finally {
			submitting = false;
		}
	}

	function looksLikeHash(s: string): boolean {
		return /^[a-fA-F0-9]{32}$|^[a-fA-F0-9]{40}$|^[a-fA-F0-9]{64}$|^[a-fA-F0-9]{128}$/.test(
			s.trim()
		);
	}

	function hashType(s: string): 'md5' | 'sha1' | 'sha256' | 'sha512' {
		const len = s.trim().length;
		if (len === 32) return 'md5';
		if (len === 40) return 'sha1';
		if (len === 128) return 'sha512';
		return 'sha256';
	}

	function clearLookup() {
		hashLookup = null;
		lookupNotFound = false;
	}

	async function submitUrlOrHash() {
		const value = urlInput.trim();
		if (!value || submitting) return;

		if (looksLikeHash(value)) {
			lookingUp = true;
			clearLookup();
			try {
				const result = await lookupSample(fetch, hashType(value), value);
				hashLookup = result;
			} catch (err) {
				if (isApiError(err) && err.status === 404) {
					lookupNotFound = true;
				} else {
					const msg = isApiError(err) ? err.message : 'Lookup failed.';
					toasts.push({ kind: 'error', message: msg });
				}
			} finally {
				lookingUp = false;
			}
			return;
		}

		submitting = true;
		try {
			const optionalFields = buildOptionalFields();
			const { task_id } = await createTaskFromUrl(fetch, { url: value, ...optionalFields });
			toasts.push({ kind: 'success', message: `Task #${task_id} submitted.` });
			await invalidate('malbox:tasks');
			urlInput = '';
			submissionConfig = defaultConfig();
			goto(`/submissions/${task_id}`);
		} catch (err) {
			const msg = isApiError(err)
				? err.status === 404
					? 'URL submission is not yet implemented on the back-end.'
					: err.message
				: 'Submission failed.';
			toasts.push({ kind: 'error', message: msg });
		} finally {
			submitting = false;
		}
	}

	async function reanalyze() {
		if (!hashLookup || submitting) return;
		submitting = true;
		try {
			const optionalFields = buildOptionalFields();
			const { task_id } = await rescanSample(fetch, hashLookup.sample.id, {
				...optionalFields,
				...(submissionConfig.vmMode !== 'no-vm' && {
					platform: submissionConfig.vmMode
				})
			});
			toasts.push({ kind: 'success', message: `Re-analysis task #${task_id} submitted.` });
			await invalidate('malbox:tasks');
			urlInput = '';
			clearLookup();
			submissionConfig = defaultConfig();
			goto(`/submissions/${task_id}`);
		} catch (err) {
			const msg = isApiError(err) ? err.message : 'Re-analysis failed.';
			toasts.push({ kind: 'error', message: msg });
		} finally {
			submitting = false;
		}
	}
</script>

<div>
	<div class="overflow-hidden rounded-2xl bg-[var(--color-bg-secondary)] p-8">
		<div class="mb-6 flex justify-center gap-2">
			<button
				onclick={() => {
					activeTab = 'file';
					clearLookup();
				}}
				class="rounded-lg px-4 py-2 transition-colors {activeTab === 'file'
					? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
			>
				File
			</button>
			<button
				onclick={() => (activeTab = 'url')}
				class="rounded-lg px-4 py-2 transition-colors {activeTab === 'url'
					? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
			>
				URL or Hash
			</button>
		</div>

		<div
			class="transition-all duration-300 ease-out"
			style="min-height: {activeTab === 'file' ? '20rem' : '120px'}"
		>
			{#if activeTab === 'file'}
				{#key activeTab}
					<div
						in:fly={{ y: 20, duration: 300, easing: cubicOut }}
						out:fly={{ y: -20, duration: 300, easing: cubicOut }}
						class="flex min-h-[20rem] flex-col items-center justify-center rounded-xl border-2 border-dashed p-16 text-center transition-colors {dragOver
							? 'border-[var(--color-accent)] bg-[var(--color-accent)]/5'
							: 'border-[var(--color-border)]'}"
						ondragover={(e) => {
							e.preventDefault();
							dragOver = true;
						}}
						ondragleave={() => (dragOver = false)}
						ondrop={onDrop}
						role="region"
					>
						<input
							type="file"
							class="hidden"
							bind:this={fileInput}
							onchange={(e) =>
								onFileChosen((e.currentTarget as HTMLInputElement).files?.[0] ?? null)}
						/>
						<svg
							class="mx-auto mb-4"
							width="38"
							height="47"
							viewBox="0 0 38 47"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
							aria-hidden="true"
						>
							<path
								d="M0 6C0 2.68629 2.68629 0 6 0H17L30 13V33C30 36.3137 27.3137 39 24 39H6C2.68629 39 0 36.3137 0 33V6Z"
								fill="#F4F4FF"
							/>
							<path d="M17 0L30 13H23C19.6863 13 17 10.3137 17 7V0Z" fill="#8A8F94" />
							<rect x="15" y="24" width="23" height="23" rx="11.5" fill="#516CF9" />
							<path
								d="M26.7041 30.0921C26.77 30.1187 26.8318 30.1588 26.8853 30.2122L29.609 32.9359C29.8217 33.1486 29.8217 33.4935 29.609 33.7063C29.3963 33.919 29.0513 33.919 28.8386 33.7063L27.0448 31.9125V37.1343C27.0448 37.4351 26.801 37.679 26.5001 37.679C26.1993 37.679 25.9554 37.4351 25.9554 37.1343V31.9125L24.1616 33.7063C23.9489 33.919 23.604 33.919 23.3912 33.7063C23.1785 33.4935 23.1785 33.1486 23.3912 32.9359L26.1129 30.2142C26.1195 30.2076 26.1262 30.2012 26.133 30.1949C26.2299 30.1066 26.3587 30.0527 26.5001 30.0527"
								fill="white"
							/>
							<path
								d="M26.5014 30.0527C26.573 30.0528 26.6414 30.0668 26.7041 30.0921L26.5014 30.0527Z"
								fill="white"
							/>
							<path
								d="M21.5975 36.5895C21.8983 36.5895 22.1422 36.8334 22.1422 37.1343V39.3132C22.1422 39.4577 22.1996 39.5962 22.3018 39.6984C22.4039 39.8005 22.5425 39.8579 22.6869 39.8579H30.3133C30.4577 39.8579 30.5963 39.8005 30.6984 39.6984C30.8006 39.5962 30.858 39.4577 30.858 39.3132V37.1343C30.858 36.8334 31.1019 36.5895 31.4027 36.5895C31.7036 36.5895 31.9475 36.8334 31.9475 37.1343V39.3132C31.9475 39.7466 31.7753 40.1623 31.4688 40.4688C31.1623 40.7752 30.7467 40.9474 30.3133 40.9474H22.6869C22.2535 40.9474 21.8379 40.7752 21.5314 40.4688C21.2249 40.1623 21.0527 39.7466 21.0527 39.3132V37.1343C21.0527 36.8334 21.2966 36.5895 21.5975 36.5895Z"
								fill="white"
							/>
						</svg>
						<p class="text-[var(--color-text-primary)]">
							Drag and drop a file here or
							<button
								class="text-[var(--color-text-primary)] underline"
								onclick={() => fileInput?.click()}>choose a file</button
							>
						</p>
						{#if selectedFile}
							<p class="mt-4 text-sm text-[var(--color-text-secondary)]">
								Selected: <span class="text-[var(--color-text-primary)]">{selectedFile.name}</span>
							</p>
							<div class="mt-4 flex items-center gap-2">
								<SubmissionConfigModal
									config={submissionConfig}
									onApply={(c) => (submissionConfig = c)}
								/>
								<button
									class="inline-flex items-center gap-2 rounded-lg bg-[var(--color-accent)] px-6 py-2 text-sm font-medium text-white transition-colors hover:bg-[var(--color-accent-hover)] disabled:opacity-50"
									disabled={submitting}
									onclick={submitFile}
								>
									{#if submitting}
										<svg
											class="h-4 w-4 animate-spin"
											viewBox="0 0 24 24"
											fill="none"
											aria-hidden="true"
										>
											<circle
												cx="12"
												cy="12"
												r="10"
												stroke="currentColor"
												stroke-width="4"
												opacity="0.25"
											/>
											<path
												d="M4 12a8 8 0 018-8"
												stroke="currentColor"
												stroke-width="4"
												stroke-linecap="round"
											/>
										</svg>
										Submitting…
									{:else}
										Submit for analysis
									{/if}
								</button>
							</div>
						{/if}
					</div>

					<div class="mt-4 flex justify-between text-sm text-[var(--color-text-secondary)]">
						<span>Supported formats: PE, ELF, Mach-O, PDF, Office, scripts, archives</span>
						<span>Maximum size: 256MB</span>
					</div>
				{/key}
			{:else}
				{#key activeTab}
					<div
						in:fly={{ y: 20, duration: 300, easing: cubicOut }}
						out:fly={{ y: -20, duration: 300, easing: cubicOut }}
					>
						<div class="mb-6 flex gap-3">
							<input
								type="text"
								bind:value={urlInput}
								oninput={clearLookup}
								placeholder="https://mywebsite.com or a file hash"
								onkeydown={(e) => {
									if (e.key === 'Enter') submitUrlOrHash();
								}}
								class="flex-1 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] px-4 py-3 text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] transition-all focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)]"
							/>
							<SubmissionConfigModal
								config={submissionConfig}
								onApply={(c) => (submissionConfig = c)}
							/>
							<button
								class="inline-flex items-center gap-2 rounded-lg bg-[var(--color-accent)] px-6 py-3 font-medium text-white transition-colors hover:bg-[var(--color-accent-hover)] disabled:opacity-50"
								disabled={submitting || lookingUp || !urlInput.trim()}
								onclick={submitUrlOrHash}
							>
								{#if submitting || lookingUp}
									<svg
										class="h-4 w-4 animate-spin"
										viewBox="0 0 24 24"
										fill="none"
										aria-hidden="true"
									>
										<circle
											cx="12"
											cy="12"
											r="10"
											stroke="currentColor"
											stroke-width="4"
											opacity="0.25"
										/>
										<path
											d="M4 12a8 8 0 018-8"
											stroke="currentColor"
											stroke-width="4"
											stroke-linecap="round"
										/>
									</svg>
									{lookingUp ? 'Looking up...' : 'Submitting…'}
								{:else}
									Submit
								{/if}
							</button>
						</div>

						<!-- Hash lookup result -->
						{#if hashLookup}
							<div
								class="mb-4 rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] p-5"
								transition:fly={{ y: 10, duration: 200, easing: cubicOut }}
							>
								<div class="mb-3 flex items-center justify-between">
									<h3 class="text-sm font-medium text-[var(--color-accent)]">
										Sample found in database
									</h3>
									<button
										type="button"
										onclick={clearLookup}
										class="text-xs text-[var(--color-text-secondary)] transition-colors hover:text-[var(--color-text-primary)]"
									>
										Dismiss
									</button>
								</div>

								<dl class="mb-4 grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1.5 text-xs">
									<dt class="text-[var(--color-text-secondary)]">Type</dt>
									<dd class="text-[var(--color-text-primary)]">
										{hashLookup.sample.file_type}
									</dd>
									<dt class="text-[var(--color-text-secondary)]">Size</dt>
									<dd class="text-[var(--color-text-primary)]">
										{formatBytes(hashLookup.sample.file_size)}
									</dd>
									<dt class="text-[var(--color-text-secondary)]">SHA-256</dt>
									<dd class="truncate font-mono text-[var(--color-text-primary)]">
										{hashLookup.sample.sha256}
									</dd>
								</dl>

								{#if hashLookup.task_ids.length > 0}
									<div class="mb-4">
										<p class="mb-2 text-xs text-[var(--color-text-secondary)]">
											Previously analyzed in {hashLookup.task_ids.length} task{hashLookup.task_ids
												.length === 1
												? ''
												: 's'}:
										</p>
										<div class="flex flex-wrap gap-2">
											{#each hashLookup.task_ids.slice(0, 10) as taskId (taskId)}
												<a
													href="/submissions/{taskId}"
													class="rounded-lg bg-[var(--color-bg-secondary)] px-3 py-1.5 text-xs font-medium text-[var(--color-accent)] transition-colors hover:bg-[var(--color-accent)]/10"
												>
													Task #{taskId}
												</a>
											{/each}
											{#if hashLookup.task_ids.length > 10}
												<span class="px-2 py-1.5 text-xs text-[var(--color-text-secondary)]">
													+{hashLookup.task_ids.length - 10} more
												</span>
											{/if}
										</div>
									</div>
								{/if}

								<button
									class="inline-flex items-center gap-2 rounded-lg bg-[var(--color-accent)] px-5 py-2 text-sm font-medium text-white transition-colors hover:bg-[var(--color-accent-hover)] disabled:opacity-50"
									disabled={submitting}
									onclick={reanalyze}
								>
									{#if submitting}
										<svg
											class="h-4 w-4 animate-spin"
											viewBox="0 0 24 24"
											fill="none"
											aria-hidden="true"
										>
											<circle
												cx="12"
												cy="12"
												r="10"
												stroke="currentColor"
												stroke-width="4"
												opacity="0.25"
											/>
											<path
												d="M4 12a8 8 0 018-8"
												stroke="currentColor"
												stroke-width="4"
												stroke-linecap="round"
											/>
										</svg>
										Submitting…
									{:else}
										Re-analyze this sample
									{/if}
								</button>
							</div>
						{/if}

						{#if lookupNotFound}
							<div
								class="mb-4 rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-tertiary)] p-4 text-sm text-[var(--color-text-secondary)]"
								transition:fly={{ y: 10, duration: 200, easing: cubicOut }}
							>
								No matching sample found in the database for this hash.
							</div>
						{/if}

						<div class="flex justify-between text-sm text-[var(--color-text-secondary)]">
							<span
								>Supported: URLs (HTTP/HTTPS, .onion) and hashes (MD5, SHA-1, SHA-256, SHA-512)</span
							>
							<span>Maximum length: 2048 characters</span>
						</div>
					</div>
				{/key}
			{/if}
		</div>
	</div>
</div>
