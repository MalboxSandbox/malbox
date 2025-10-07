<script lang="ts">
	import FileUpload from '$lib/components/FileUpload.svelte';
	import SubmissionStats from '$lib/components/SubmissionStats.svelte';
	import SystemStats from '$lib/components/SystemStats.svelte';
</script>

<div class="max-w-7xl mx-auto flex flex-col gap-8">
	<div class="text-center space-y-3">
		<h1 class="text-3xl font-semibold text-[var(--color-text-primary)]">
			Upload your File, URL or Hash
		</h1>
		<p class="text-[var(--color-text-secondary)]">
			Upload your sample and get started with analysis, or just insert a hash or URL to match with
			your IOC database!
		</p>
	</div>

	<FileUpload />

	<!-- Statistics Section -->
	<div class="grid grid-cols-1 lg:grid-cols-2 gap-8">
		<SubmissionStats />
		<SystemStats />
	</div>
</div>
