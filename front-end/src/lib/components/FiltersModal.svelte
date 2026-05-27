<script lang="ts">
	interface Props {
		isOpen: boolean;
		onClose: () => void;
		onApply: (filters: FilterValues) => void;
	}

	interface FilterValues {
		scoreRange: [number, number];
		user: string | null;
		date: string | null;
		type: 'file' | 'url' | 'hash' | null;
		fileType: string | null;
	}

	let { isOpen, onClose, onApply }: Props = $props();

	// Filter state
	let scoreMin = $state(0);
	let scoreMax = $state(7);
	let selectedUser = $state<string | null>(null);
	let selectedDate = $state<string | null>(null);
	let selectedType = $state<'file' | 'url' | 'hash' | null>(null);
	let selectedFileType = $state<string | null>(null);

	function handleCancel() {
		onClose();
	}

	function handleConfirm() {
		onApply({
			scoreRange: [scoreMin, scoreMax],
			user: selectedUser,
			date: selectedDate,
			type: selectedType,
			fileType: selectedFileType
		});
		onClose();
	}

	function selectType(type: 'file' | 'url' | 'hash') {
		selectedType = selectedType === type ? null : type;
	}
</script>

{#if isOpen}
	<!-- Backdrop -->
	<div
		class="fixed inset-0 bg-black/50 z-40"
		onclick={onClose}
		onkeydown={(e) => {
			if (e.key === 'Enter' || e.key === ' ') onClose();
		}}
		role="button"
		tabindex="-1"
	></div>

	<!-- Modal -->
	<div class="fixed top-24 right-8 z-50 w-full max-w-md">
		<div class="bg-[var(--color-bg-card)] rounded-xl shadow-2xl p-6 space-y-6">
			<!-- Score Range -->
			<div class="space-y-3">
				<h3 class="text-[var(--color-text-primary)] text-base font-medium">Score range</h3>
				<div class="space-y-2">
					<div class="flex items-center justify-between text-sm">
						<span class="text-[var(--color-text-secondary)]">{scoreMin}</span>
						<span class="text-[var(--color-text-secondary)]">{scoreMax}</span>
					</div>
					<div class="relative h-2">
						<!-- Track -->
						<div class="absolute inset-0 bg-[var(--color-border)] rounded-full"></div>
						<!-- Active range -->
						<div
							class="absolute h-full bg-[var(--color-accent)] rounded-full"
							style="left: {(scoreMin / 10) * 100}%; right: {100 - (scoreMax / 10) * 100}%"
						></div>
						<!-- Min handle -->
						<input
							type="range"
							min="0"
							max="10"
							bind:value={scoreMin}
							class="absolute w-full h-2 appearance-none bg-transparent pointer-events-auto cursor-pointer range-slider"
							style="z-index: {scoreMin > scoreMax - 1 ? 2 : 1}"
						/>
						<!-- Max handle -->
						<input
							type="range"
							min="0"
							max="10"
							bind:value={scoreMax}
							class="absolute w-full h-2 appearance-none bg-transparent pointer-events-auto cursor-pointer range-slider"
							style="z-index: {scoreMax < scoreMin + 1 ? 2 : 1}"
						/>
					</div>
				</div>
			</div>

			<!-- User -->
			<div class="space-y-3">
				<h3 class="text-[var(--color-text-primary)] text-base font-medium">User</h3>
				<div class="relative">
					<select
						bind:value={selectedUser}
						class="w-full px-4 py-3 bg-[var(--color-bg-tertiary)] border border-[var(--color-border)] rounded-lg text-[var(--color-text-secondary)] text-sm appearance-none cursor-pointer focus:outline-none focus:border-[var(--color-accent)] transition-colors"
					>
						<option value={null}>Choose the user</option>
						<option value="user1">User 1</option>
						<option value="user2">User 2</option>
						<option value="user3">User 3</option>
					</select>
					<svg
						class="absolute right-4 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--color-text-secondary)] pointer-events-none"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
					>
						<polyline points="6 9 12 15 18 9" />
					</svg>
				</div>
			</div>

			<!-- Date -->
			<div class="space-y-3">
				<h3 class="text-[var(--color-text-primary)] text-base font-medium">Date</h3>
				<div class="relative">
					<input
						type="date"
						bind:value={selectedDate}
						class="w-full px-4 py-3 bg-[var(--color-bg-tertiary)] border border-[var(--color-border)] rounded-lg text-[var(--color-text-secondary)] text-sm cursor-pointer focus:outline-none focus:border-[var(--color-accent)] transition-colors [&::-webkit-calendar-picker-indicator]:opacity-0"
						placeholder="Choose the date"
					/>
					<svg
						class="absolute right-4 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--color-text-secondary)] pointer-events-none"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
					>
						<rect x="3" y="4" width="18" height="18" rx="2" ry="2" />
						<line x1="16" y1="2" x2="16" y2="6" />
						<line x1="8" y1="2" x2="8" y2="6" />
						<line x1="3" y1="10" x2="21" y2="10" />
					</svg>
				</div>
			</div>

			<!-- Type -->
			<div class="space-y-3">
				<h3 class="text-[var(--color-text-primary)] text-base font-medium">Type</h3>
				<div class="grid grid-cols-3 gap-3">
					<button
						onclick={() => selectType('file')}
						class="px-4 py-2.5 rounded-lg text-sm font-medium transition-colors {selectedType ===
						'file'
							? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
							: 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
					>
						File
					</button>
					<button
						onclick={() => selectType('url')}
						class="px-4 py-2.5 rounded-lg text-sm font-medium transition-colors {selectedType ===
						'url'
							? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
							: 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
					>
						URL
					</button>
					<button
						onclick={() => selectType('hash')}
						class="px-4 py-2.5 rounded-lg text-sm font-medium transition-colors {selectedType ===
						'hash'
							? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
							: 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]'}"
					>
						Hash
					</button>
				</div>
			</div>

			<!-- File Type -->
			<div class="space-y-3">
				<h3 class="text-[var(--color-text-primary)] text-base font-medium">File type</h3>
				<div class="relative">
					<select
						bind:value={selectedFileType}
						class="w-full px-4 py-3 bg-[var(--color-bg-tertiary)] border border-[var(--color-border)] rounded-lg text-[var(--color-text-secondary)] text-sm appearance-none cursor-pointer focus:outline-none focus:border-[var(--color-accent)] transition-colors"
					>
						<option value={null}>Choose the file type</option>
						<option value="exe">Executable (.exe)</option>
						<option value="pdf">PDF (.pdf)</option>
						<option value="doc">Document (.doc, .docx)</option>
						<option value="zip">Archive (.zip, .rar)</option>
					</select>
					<svg
						class="absolute right-4 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--color-text-secondary)] pointer-events-none"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
					>
						<polyline points="6 9 12 15 18 9" />
					</svg>
				</div>
			</div>

			<!-- Actions -->
			<div class="flex items-center gap-3 pt-4">
				<button
					onclick={handleCancel}
					class="flex-1 px-6 py-3 bg-[var(--color-bg-tertiary)] hover:bg-[var(--color-bg-secondary)] text-[var(--color-text-primary)] text-sm font-medium rounded-lg transition-colors"
				>
					Cancel
				</button>
				<button
					onclick={handleConfirm}
					class="flex-1 px-6 py-3 bg-[var(--color-accent)] hover:bg-[var(--color-accent-hover)] text-[var(--color-text-primary)] text-sm font-medium rounded-lg transition-colors"
				>
					Confirm
				</button>
			</div>
		</div>
	</div>
{/if}

<style>
	/* Custom range slider styling */
	.range-slider::-webkit-slider-thumb {
		appearance: none;
		width: 18px;
		height: 18px;
		border-radius: 50%;
		background: var(--color-accent);
		cursor: pointer;
		border: 3px solid var(--color-bg-card);
		box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
	}

	.range-slider::-moz-range-thumb {
		width: 18px;
		height: 18px;
		border-radius: 50%;
		background: var(--color-accent);
		cursor: pointer;
		border: 3px solid var(--color-bg-card);
		box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
	}
</style>
