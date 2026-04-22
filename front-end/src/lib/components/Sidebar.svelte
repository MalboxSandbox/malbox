<script lang="ts">
	import { page } from '$app/stores';
	import Logo from '$lib/components/ui/Logo.svelte';
	import Icon from '$lib/components/ui/Icon.svelte';
	import { auth } from '$lib/stores/auth.svelte';
	import { icons, type IconName } from '$lib/icons';
	import type { TaskReport } from '$lib/api/types';

	const navItems: { href: string; icon: IconName; label: string }[] = [
		{ href: '/dashboard', icon: 'home', label: 'Home' },
		{ href: '/submissions', icon: 'upload', label: 'Submissions' },
		{ href: '/machines', icon: 'machines', label: 'Machines' },
		{ href: '/images', icon: 'images', label: 'Images' },
		{ href: '/marketplace', icon: 'marketplace', label: 'Marketplace' },
		{ href: '/automation', icon: 'api', label: 'Automation API' }
	];

	const isActive = (href: string) => $page.url.pathname === href;

	// Submission context — present only on /submissions/[id]/... routes.
	const report = $derived($page.data?.report as TaskReport | undefined);
	const summaryHref = $derived(report ? `/submissions/${report.task.id}` : null);
	const activePlugin = $derived.by(() => {
		const m = $page.url.pathname.match(/^\/submissions\/\d+\/p\/([^/]+)/);
		return m ? decodeURIComponent(m[1]) : null;
	});
	const onSummary = $derived(
		summaryHref !== null && $page.url.pathname === summaryHref
	);
</script>

<aside
	class="w-64 h-screen bg-[var(--color-bg-secondary)] border-r border-[var(--color-border)] flex flex-col fixed left-0 top-0"
>
	<!-- Logo -->
	<div class="p-6 border-b border-[var(--color-border)] shrink-0">
		<Logo iconSize="w-8 h-8" />
	</div>

	<!-- Navigation -->
	<nav class="flex-1 min-h-0 overflow-y-auto px-3 pt-4 pb-4">
		<div class="text-xs font-medium text-[var(--color-text-secondary)] px-3 mb-3">Navigation</div>

		<div class="space-y-2">
			{#each navItems as item (item.href)}
				<a
					href={item.href}
					class="flex items-center gap-3 px-3 py-2 rounded-lg transition-colors
					       {isActive(item.href) ? 'text-[#F4F4FF]' : 'text-[#8A8F94] hover:text-[#F4F4FF]'}"
				>
					<div
						class="w-10 h-10 rounded-xl flex items-center justify-center transition-colors
						       {isActive(item.href)
							? 'bg-[#1D2342] text-[#516CF9]'
							: 'bg-[#25272C] text-[#8A8F94]'}"
					>
						<Icon path={icons[item.icon]} />
					</div>
					<span class="text-sm font-medium">{item.label}</span>
				</a>
			{/each}
		</div>

		{#if report && summaryHref}
			<div class="text-xs font-medium text-[var(--color-text-secondary)] px-3 mt-6 mb-3">
				Submission
			</div>
			<div class="px-3 pb-3 text-xs text-[var(--color-text-primary)] truncate font-mono">
				{report.task.target}
			</div>
			<div class="space-y-2">
				<a
					href={summaryHref}
					class="flex items-center gap-3 px-3 py-2 rounded-lg transition-colors
					       {onSummary ? 'text-[#F4F4FF]' : 'text-[#8A8F94] hover:text-[#F4F4FF]'}"
				>
					<div
						class="w-10 h-10 rounded-xl flex items-center justify-center transition-colors
						       {onSummary
							? 'bg-[#1D2342] text-[#516CF9]'
							: 'bg-[#25272C] text-[#8A8F94]'}"
					>
						<Icon path={icons.summary} />
					</div>
					<span class="text-sm font-medium">Summary</span>
				</a>
				{#each report.plugins as p (p.plugin_name)}
					{@const displayName =
						p.report?.plugin.display_name ?? p.report?.plugin.id ?? p.plugin_name}
					{@const href = `${summaryHref}/p/${encodeURIComponent(p.plugin_name)}`}
					{@const active = activePlugin === p.plugin_name}
					<div>
						<a
							{href}
							class="flex items-center gap-3 px-3 py-2 rounded-lg transition-colors
							       {active ? 'text-[#F4F4FF]' : 'text-[#8A8F94] hover:text-[#F4F4FF]'}"
						>
							<div
								class="w-10 h-10 rounded-xl flex items-center justify-center transition-colors
								       {active
									? 'bg-[#1D2342] text-[#516CF9]'
									: 'bg-[#25272C] text-[#8A8F94]'}"
							>
								<Icon path={icons.plugin} />
							</div>
							<span class="text-sm font-medium truncate">{displayName}</span>
						</a>
						{#if active && p.report?.sections && p.report.sections.length > 0}
							<div class="ml-8 mt-2 mb-1 space-y-1 border-l border-[var(--color-border)] pl-6">
								{#each p.report.sections as s (s.id)}
									{@const secHref = `${href}#section-${s.id}`}
									{@const secActive = $page.url.hash === `#section-${s.id}`}
									<a
										href={secHref}
										class="block py-1.5 rounded text-sm transition-colors
										       {secActive
											? 'text-[var(--color-text-primary)]'
											: 'text-[#8A8F94] hover:text-[#F4F4FF]'}"
									>
										{s.title}
									</a>
								{/each}
							</div>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	</nav>

	<!-- User Profile -->
	<div class="p-4 border-t border-[var(--color-border)] shrink-0">
		<div class="flex items-center gap-3">
			<div
				class="w-7 h-7 rounded-full bg-[var(--color-accent)] flex items-center justify-center text-white font-semibold"
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
	</div>
</aside>
