<script lang="ts">
	import Icon from '$lib/components/ui/Icon.svelte';
	import { icons } from '$lib/icons';

	const sections: { href: string; title: string; description: string; icon: keyof typeof icons }[] =
		[
			{
				href: '/settings/machines',
				title: 'Machines',
				description: 'Virtual machines registered with the analysis platform.',
				icon: 'machines'
			},
			{
				href: '/settings/images',
				title: 'Images',
				description: 'Disk images used to provision analysis machines.',
				icon: 'images'
			},
			{
				href: '/settings/plugins',
				title: 'Installed plugins',
				description: 'Guest and host plugins registered with the analysis daemon.',
				icon: 'plugin'
			},
			{
				href: '/settings/transforms',
				title: 'Transforms & Recipes',
				description:
					'Manage custom transform definitions and reusable pipelines for the workbench.',
				icon: 'workbench'
			},
			{
				href: '/settings/context-menu',
				title: 'Context Menu',
				description:
					'Configure the right-click context menu - lookup providers, built-in actions, defang format, and custom actions.',
				icon: 'providers'
			}
		];
</script>

<div class="mx-auto max-w-4xl space-y-6">
	<h1 class="text-3xl font-semibold text-[var(--color-text-primary)]">Settings</h1>

	<div class="grid grid-cols-1 gap-4 md:grid-cols-2">
		{#each sections as s (s.href)}
			<a
				href={s.href}
				class="group flex items-start gap-4 rounded-2xl bg-[var(--color-bg-secondary)] p-6 transition-colors hover:bg-[var(--color-bg-tertiary)]"
			>
				<div
					class="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] transition-colors group-hover:bg-[var(--color-bg-card)] group-hover:text-[var(--color-accent)]"
				>
					<Icon path={icons[s.icon]} class="size-5" />
				</div>
				<div class="min-w-0">
					<h2 class="text-lg font-semibold text-[var(--color-text-primary)]">{s.title}</h2>
					<p class="mt-1 text-sm text-[var(--color-text-secondary)]">{s.description}</p>
				</div>
			</a>
		{/each}
	</div>
</div>
