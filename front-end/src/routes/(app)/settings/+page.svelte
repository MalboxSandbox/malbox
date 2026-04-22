<script lang="ts">
	const sections = [
		{
			href: '/settings/plugins',
			title: 'Installed plugins',
			description: 'Guest and host plugins registered with the analysis daemon.'
		}
	];
</script>

<div class="mx-auto max-w-4xl space-y-6">
	<h1 class="text-3xl font-semibold text-[var(--color-text-primary)]">Settings</h1>

	<div class="grid grid-cols-1 gap-4 md:grid-cols-2">
		{#each sections as s}
			<a
				href={s.href}
				class="block rounded-2xl bg-[var(--color-bg-secondary)] p-6 transition-colors hover:bg-[var(--color-bg-tertiary)]"
			>
				<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">{s.title}</h2>
				<p class="mt-2 text-sm text-[var(--color-text-secondary)]">{s.description}</p>
			</a>
		{/each}
	</div>
</div>
