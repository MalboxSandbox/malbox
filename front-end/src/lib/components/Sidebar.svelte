<script lang="ts">
	import { page } from '$app/stores';
	import Logo from '$lib/components/ui/Logo.svelte';
	import Icon from '$lib/components/ui/Icon.svelte';
	import { auth } from '$lib/stores/auth.svelte';
	import { sidebar } from '$lib/stores/sidebar.svelte';
	import { reportStore } from '$lib/stores/report.svelte';
	import { icons, type IconName } from '$lib/icons';
	import type { Section } from '$lib/api/types';

	function isArtifactSection(s: Section, artifactNames: Set<string>): boolean {
		if (s.blocks && s.blocks.length === 1) {
			const t = s.blocks[0].type;
			if (t === 'download' || t === 'json') return true;
		}
		return artifactNames.has(s.title);
	}

	const navItems: { href: string; icon: IconName; label: string }[] = [
		{ href: '/dashboard', icon: 'home', label: 'Home' },
		{ href: '/submissions', icon: 'upload', label: 'Submissions' },
		{ href: '/marketplace', icon: 'marketplace', label: 'Marketplace' },
		{ href: '/automation', icon: 'api', label: 'Automation API' },
		{ href: '/workbench', icon: 'workbench', label: 'Workbench' }
	];

	const isActive = (href: string) => $page.url.pathname === href;

	const report = $derived(reportStore.current);
	const successfulPlugins = $derived(report?.plugins.filter((p) => !p.failed) ?? []);
	const failedPlugins = $derived(report?.plugins.filter((p) => p.failed) ?? []);
	const failedNames = $derived(
		failedPlugins.map((p) => p.report?.plugin.display_name ?? p.report?.plugin.id ?? p.plugin_name)
	);
	let failedExpanded = $state(false);
	const summaryHref = $derived(report ? `/submissions/${report.task.id}` : null);
	const activePlugin = $derived.by(() => {
		const m = $page.url.pathname.match(/^\/submissions\/\d+\/p\/([^/]+)/);
		return m ? decodeURIComponent(m[1]) : null;
	});
	const onSummary = $derived(summaryHref !== null && $page.url.pathname === summaryHref);
</script>

<aside
	class="group/sidebar h-screen bg-[var(--color-bg-secondary)] border-r border-[var(--color-border)] flex flex-col fixed left-0 top-0 transition-[width] duration-300 ease-out z-30
		{sidebar.collapsed ? 'w-[72px]' : 'w-64'}"
>
	<!-- Toggle button -->
	<button
		type="button"
		onclick={() => sidebar.toggle()}
		class="absolute -right-3 top-1/2 -translate-y-1/2 z-40
			w-5 h-14 rounded-lg overflow-hidden
			bg-[var(--color-bg-secondary)] border border-[var(--color-border)]
			flex items-center justify-center
			text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:border-[var(--color-text-secondary)]/30
			opacity-0 group-hover/sidebar:opacity-100
			transition-all duration-200 hover:scale-105 active:scale-95
			shadow-sm cursor-pointer"
		title={sidebar.collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
	>
		<div
			class="absolute inset-x-0 top-1 bottom-1"
			style="background-image: repeating-linear-gradient(
				-45deg,
				var(--color-border) 0px,
				var(--color-border) 1px,
				transparent 1px,
				transparent 5px
			);
			mask-image: radial-gradient(ellipse at center, black 0%, transparent 75%);"
		></div>
		<svg
			xmlns="http://www.w3.org/2000/svg"
			viewBox="0 0 20 20"
			fill="currentColor"
			class="relative size-3 transition-transform duration-300 ease-out {sidebar.collapsed
				? 'rotate-180'
				: ''}"
		>
			<path
				fill-rule="evenodd"
				d="M11.78 5.22a.75.75 0 0 1 0 1.06L8.06 10l3.72 3.72a.75.75 0 1 1-1.06 1.06l-4.25-4.25a.75.75 0 0 1 0-1.06l4.25-4.25a.75.75 0 0 1 1.06 0Z"
				clip-rule="evenodd"
			/>
		</svg>
	</button>

	<!-- Logo -->
	<div
		class="p-6 border-b border-[var(--color-border)] shrink-0 {sidebar.collapsed
			? 'flex justify-center'
			: ''}"
	>
		<Logo iconSize="w-8 h-8" showText={!sidebar.collapsed} />
	</div>

	<!-- Navigation -->
	<nav class="flex-1 min-h-0 overflow-y-auto pt-4 pb-4 {sidebar.collapsed ? 'px-2' : 'px-3'}">
		{#if !sidebar.collapsed}
			<div class="text-xs font-medium text-[var(--color-text-secondary)] px-3 mb-3">Navigation</div>
		{/if}

		<div class="space-y-2">
			{#each navItems as item (item.href)}
				<a
					href={item.href}
					class="flex items-center gap-3 px-3 py-2 rounded-lg transition-colors
					       {sidebar.collapsed ? 'justify-center px-0' : ''}
					       {isActive(item.href) ? 'text-[#F4F4FF]' : 'text-[#8A8F94] hover:text-[#F4F4FF]'}"
					title={sidebar.collapsed ? item.label : undefined}
				>
					<div
						class="w-10 h-10 shrink-0 rounded-xl flex items-center justify-center transition-colors
						       {isActive(item.href) ? 'bg-[#1D2342] text-[#516CF9]' : 'bg-[#25272C] text-[#8A8F94]'}"
					>
						<Icon path={icons[item.icon]} />
					</div>
					{#if !sidebar.collapsed}
						<span class="text-sm font-medium">{item.label}</span>
					{/if}
				</a>
			{/each}
		</div>

		{#if report && summaryHref}
			{#if !sidebar.collapsed}
				<div class="text-xs font-medium text-[var(--color-text-secondary)] px-3 mt-6 mb-3">
					Submission
				</div>
			{:else}
				<div class="my-3 mx-2 border-t border-[var(--color-border)]"></div>
			{/if}
			<div class="space-y-2">
				<a
					href={summaryHref}
					class="flex items-center gap-3 px-3 py-2 rounded-lg transition-colors
					       {sidebar.collapsed ? 'justify-center px-0' : ''}
					       {onSummary ? 'text-[#F4F4FF]' : 'text-[#8A8F94] hover:text-[#F4F4FF]'}"
					title={sidebar.collapsed ? 'Summary' : undefined}
				>
					<div
						class="w-10 h-10 rounded-xl flex items-center justify-center transition-colors shrink-0
						       {onSummary ? 'bg-[#1D2342] text-[#516CF9]' : 'bg-[#25272C] text-[#8A8F94]'}"
					>
						<Icon path={icons.summary} />
					</div>
					{#if !sidebar.collapsed}
						<div class="min-w-0">
							<span class="text-sm font-medium">Summary</span>
							<div
								class="truncate text-xs text-[var(--color-text-secondary)]"
								title={report.task.target}
							>
								{report.task.target}
							</div>
						</div>
					{/if}
				</a>
				{#each successfulPlugins as p (p.plugin_name)}
					{@const displayName =
						p.report?.plugin.display_name ?? p.report?.plugin.id ?? p.plugin_name}
					{@const href = `${summaryHref}/p/${encodeURIComponent(p.plugin_name)}`}
					{@const active = activePlugin === p.plugin_name}
					<div>
						<a
							{href}
							class="flex items-center gap-3 px-3 py-2 rounded-lg transition-colors
							       {sidebar.collapsed ? 'justify-center px-0' : ''}
							       {active ? 'text-[#F4F4FF]' : 'text-[#8A8F94] hover:text-[#F4F4FF]'}"
							title={sidebar.collapsed ? displayName : undefined}
						>
							<div
								class="w-10 h-10 shrink-0 rounded-xl flex items-center justify-center transition-colors
								       {active ? 'bg-[#1D2342] text-[#516CF9]' : 'bg-[#25272C] text-[#8A8F94]'}"
							>
								<Icon path={icons.plugin} />
							</div>
							{#if !sidebar.collapsed}
								<span class="text-sm font-medium truncate">{displayName}</span>
							{/if}
						</a>
						{#if !sidebar.collapsed && active && p.report?.sections && p.report.sections.filter((s) => !p.synthesized || !isArtifactSection(s, new Set(p.artifacts.map((a) => a.result_name)))).length > 0}
							<div class="ml-8 mt-2 mb-1 space-y-1 border-l border-[var(--color-border)] pl-6">
								{#each p.report.sections.filter((s) => !p.synthesized || !isArtifactSection(s, new Set(p.artifacts.map((a) => a.result_name)))) as s (s.id)}
									{@const secHref = `${href}#section-${s.id}`}
									{@const secActive = $page.url.hash === `#section-${s.id}`}
									<a
										href={secHref}
										class="block py-1.5 rounded text-sm truncate transition-colors
										       {secActive ? 'text-[var(--color-text-primary)]' : 'text-[#8A8F94] hover:text-[#F4F4FF]'}"
										title={s.title}
									>
										{s.title}
									</a>
								{/each}
							</div>
						{/if}
					</div>
				{/each}
				{#if failedPlugins.length > 0}
					<div>
						<button
							type="button"
							onclick={() => (failedExpanded = !failedExpanded)}
							class="flex w-full items-center gap-3 px-3 py-2 rounded-lg text-[#8A8F94] hover:text-[#F4F4FF] transition-colors cursor-pointer
							       {sidebar.collapsed ? 'justify-center px-0' : ''}"
							title={sidebar.collapsed
								? `${failedPlugins.length} failed: ${failedNames.join(', ')}`
								: undefined}
						>
							<div
								class="relative w-10 h-10 shrink-0 rounded-xl flex items-center justify-center bg-red-500/10 text-red-400"
							>
								<svg
									xmlns="http://www.w3.org/2000/svg"
									viewBox="0 0 16 16"
									fill="currentColor"
									class="size-4"
								>
									<path
										fill-rule="evenodd"
										d="M6.701 2.25c.577-1 2.02-1 2.598 0l5.196 9a1.5 1.5 0 0 1-1.299 2.25H2.804a1.5 1.5 0 0 1-1.3-2.25l5.197-9ZM8 4a.75.75 0 0 1 .75.75v3a.75.75 0 0 1-1.5 0v-3A.75.75 0 0 1 8 4Zm0 8a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z"
										clip-rule="evenodd"
									/>
								</svg>
								<span
									class="absolute -top-1 -right-1 flex h-4 min-w-4 items-center justify-center rounded-full bg-red-500 px-1 text-[10px] font-semibold text-white"
								>
									{failedPlugins.length}
								</span>
							</div>
							{#if !sidebar.collapsed}
								<span class="text-sm font-medium truncate text-[#8A8F94]">Failed</span>
								<svg
									xmlns="http://www.w3.org/2000/svg"
									viewBox="0 0 20 20"
									fill="currentColor"
									class="ml-auto size-4 shrink-0 text-[#8A8F94] transition-transform duration-200 {failedExpanded
										? 'rotate-180'
										: ''}"
								>
									<path
										fill-rule="evenodd"
										d="M5.22 8.22a.75.75 0 0 1 1.06 0L10 11.94l3.72-3.72a.75.75 0 1 1 1.06 1.06l-4.25 4.25a.75.75 0 0 1-1.06 0L5.22 9.28a.75.75 0 0 1 0-1.06Z"
										clip-rule="evenodd"
									/>
								</svg>
							{/if}
						</button>
						{#if !sidebar.collapsed && failedExpanded}
							<div class="ml-8 mt-1 mb-1 space-y-0.5 border-l border-red-500/20 pl-6">
								{#each failedNames as name (name)}
									<div class="py-1 text-xs truncate text-[#8A8F94]" title={name}>
										{name}
									</div>
								{/each}
							</div>
						{/if}
					</div>
				{/if}
			</div>
		{/if}
	</nav>

	<!-- User Profile -->
	<div class="border-t border-[var(--color-border)] shrink-0 {sidebar.collapsed ? 'p-3' : 'p-4'}">
		{#if sidebar.collapsed}
			<div class="flex justify-center">
				<div
					class="w-8 h-8 rounded-full bg-[var(--color-accent)] flex items-center justify-center text-white text-xs font-semibold leading-none"
				>
					{auth.user?.name?.charAt(0).toUpperCase() || 'U'}
				</div>
			</div>
		{:else}
			<div class="flex items-center gap-3">
				<div
					class="w-8 h-8 shrink-0 rounded-full bg-[var(--color-accent)] flex items-center justify-center text-white text-xs font-semibold leading-none"
				>
					{auth.user?.name?.charAt(0).toUpperCase() || 'U'}
				</div>
				<div class="flex-1 min-w-0">
					<div class="text-sm font-medium text-[var(--color-text-primary)] truncate">
						{auth.user?.name || 'User'}
					</div>
				</div>
				<a
					href="/settings"
					class="p-2 hover:bg-[var(--color-bg-tertiary)] rounded-lg transition-colors"
					aria-label="Settings"
				>
					<Icon path={icons.settings} class="w-5 h-5 text-[var(--color-text-secondary)]" />
				</a>
				<button
					onclick={() => auth.logout()}
					class="p-2 hover:bg-[var(--color-bg-tertiary)] rounded-lg transition-colors"
					aria-label="Logout"
					title="Logout"
				>
					<svg
						class="w-5 h-5 text-[var(--color-text-secondary)]"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
					>
						<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
						<polyline points="16 17 21 12 16 7" />
						<line x1="21" y1="12" x2="9" y2="12" />
					</svg>
				</button>
			</div>
		{/if}
	</div>
</aside>
