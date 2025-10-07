<script lang="ts">
	import { fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';

	let activeTab = $state<'file' | 'url'>('file');
	let urlInput = $state('');
</script>

<div>
	<div class="bg-[var(--color-bg-secondary)] rounded-2xl p-8 overflow-hidden">
		<!-- Tabs -->
		<div class="flex gap-2 mb-6">
			<button
				onclick={() => (activeTab = 'file')}
				class="px-4 py-2 rounded-lg transition-colors {activeTab === 'file'
					? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:cursor-pointer'}"
			>
				File
			</button>
			<button
				onclick={() => (activeTab = 'url')}
				class="px-4 py-2 rounded-lg transition-colors {activeTab === 'url'
					? 'bg-[var(--color-tab-active)] text-[var(--color-text-primary)]'
					: 'text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:cursor-pointer'}"
			>
				URL or Hash
			</button>
		</div>

		<!-- Upload Area with smooth height transition -->
		<div
			class="transition-all duration-300 ease-out"
			style="min-height: {activeTab === 'file' ? '300px' : '120px'}"
		>
			{#if activeTab === 'file'}
				{#key activeTab}
					<div
						in:fly={{ y: 20, duration: 300, easing: cubicOut }}
						out:fly={{ y: -20, duration: 300, easing: cubicOut }}
						class="border-2 border-dashed border-[var(--color-border)] rounded-xl p-16 text-center"
					>
						<div class="mb-4">
							<svg
								class="w-12 h-12 mx-auto mb-4"
								viewBox="0 0 39 47"
								fill="none"
								xmlns="http://www.w3.org/2000/svg"
							>
								<path
									d="M0.5 6C0.5 2.68629 3.18629 0 6.5 0H17.5L30.5 13V33C30.5 36.3137 27.8137 39 24.5 39H6.5C3.18629 39 0.5 36.3137 0.5 33V6Z"
									fill="#F4F4FF"
								/>
								<path d="M17.5 0L30.5 13H23.5C20.1863 13 17.5 10.3137 17.5 7V0Z" fill="#8A8F94" />
								<rect x="15.5" y="24" width="23" height="23" rx="11.5" fill="#516CF9" />
								<path
									d="M27.2041 30.0922C27.27 30.1188 27.3318 30.1588 27.3853 30.2123L30.109 32.936C30.3217 33.1487 30.3217 33.4936 30.109 33.7063C29.8963 33.9191 29.5513 33.9191 29.3386 33.7063L27.5448 31.9126V37.1343C27.5448 37.4352 27.301 37.6791 27.0001 37.6791C26.6993 37.6791 26.4554 37.4352 26.4554 37.1343V31.9126L24.6616 33.7063C24.4489 33.9191 24.104 33.9191 23.8912 33.7063C23.6785 33.4936 23.6785 33.1487 23.8912 32.936L26.6129 30.2143C26.6195 30.2077 26.6262 30.2013 26.633 30.195C26.7299 30.1066 26.8587 30.0527 27.0001 30.0527"
									fill="white"
								/>
								<path
									d="M27.0014 30.0527C27.073 30.0529 27.1414 30.0669 27.2041 30.0922L27.0014 30.0527Z"
									fill="white"
								/>
								<path
									d="M22.0975 36.5896C22.3983 36.5896 22.6422 36.8335 22.6422 37.1343V39.3133C22.6422 39.4577 22.6996 39.5963 22.8018 39.6984C22.9039 39.8006 23.0425 39.858 23.1869 39.858H30.8133C30.9577 39.858 31.0963 39.8006 31.1984 39.6984C31.3006 39.5963 31.358 39.4577 31.358 39.3133V37.1343C31.358 36.8335 31.6019 36.5896 31.9027 36.5896C32.2036 36.5896 32.4475 36.8335 32.4475 37.1343V39.3133C32.4475 39.7467 32.2753 40.1623 31.9688 40.4688C31.6623 40.7753 31.2467 40.9475 30.8133 40.9475H23.1869C22.7535 40.9475 22.3379 40.7753 22.0314 40.4688C21.7249 40.1623 21.5527 39.7467 21.5527 39.3133V37.1343C21.5527 36.8335 21.7966 36.5896 22.0975 36.5896Z"
									fill="white"
								/>
							</svg>
						</div>
						<p class="text-[var(--color-text-primary)] mb-1">
							Drag and drop a file here <button class="text-[var(--color-accent)] underline"
								>or choose a file</button
							>
						</p>
					</div>

					<div class="flex justify-between mt-4 text-sm text-[var(--color-text-secondary)]">
						<span>Supported formats: ELF, PE, OLE and more!</span>
						<span>Maximum size: 3GB</span>
					</div>
				{/key}
			{:else}
				{#key activeTab}
					<div
						in:fly={{ y: 20, duration: 300, easing: cubicOut }}
						out:fly={{ y: -20, duration: 300, easing: cubicOut }}
					>
						<div class="flex gap-3 mb-6">
							<input
								type="text"
								bind:value={urlInput}
								placeholder="https://mywebsite.com"
								class="flex-1 px-4 py-3 bg-[var(--color-bg-tertiary)] border border-[var(--color-border)]
                         rounded-lg text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)]
                         focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)] transition-all"
							/>
							<button
								class="px-6 py-3 bg-[var(--color-accent)] hover:bg-[var(--color-accent-hover)]
                         text-white font-medium rounded-lg transition-colors"
							>
								Submit
							</button>
						</div>

						<div class="flex justify-between text-sm text-[var(--color-text-secondary)]">
							<span>Supported: URLs (HTTP/HTTPS, .onion) and hashes (MD5, SHA-256, ...)</span>
							<span>Maximum length: 2048 characters</span>
						</div>
					</div>
				{/key}
			{/if}
		</div>
	</div>
</div>
