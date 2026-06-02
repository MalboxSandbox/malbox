<script lang="ts">
	import { page } from '$app/stores';
	import Logo from '$lib/components/ui/Logo.svelte';
	import Icon from '$lib/components/ui/Icon.svelte';
	import { auth } from '$lib/stores/auth.svelte';
	import { sidebar } from '$lib/stores/sidebar.svelte';
	import { reportStore } from '$lib/stores/report.svelte';
	import { icons, type IconName } from '$lib/icons';

	const navItems: { href: string; icon: IconName; label: string }[] = [
		{ href: '/dashboard', icon: 'home', label: 'Home' },
		{ href: '/submissions', icon: 'upload', label: 'Submissions' },
		{ href: '/marketplace', icon: 'marketplace', label: 'Marketplace' },
		{ href: '/automation', icon: 'api', label: 'Automation API' },
		{ href: '/workbench', icon: 'workbench', label: 'Workbench' }
	];

	const reportSections: { id: string; label: string; icon: IconName }[] = [
		{ id: 'overview', label: 'Overview', icon: 'summary' },
		{ id: 'verdicts', label: 'Verdicts', icon: 'chart' }
	];

	const isActive = (href: string) => $page.url.pathname === href;

	const samplePage = $derived(reportStore.samplePage);
	const activePlugins = $derived(reportStore.activePlugins);

	const successfulPlugins = $derived(activePlugins.filter((p) => !p.failed));

	const dynamicSections = $derived.by(() => {
		const sections = [...reportSections];
		for (const p of successfulPlugins) {
			if (p.report?.sections && p.report.sections.length > 0) {
				const name = p.report.plugin.display_name ?? p.report.plugin.id ?? p.plugin_name;
				sections.push({ id: `plugin-${p.plugin_name}`, label: name, icon: 'plugin' as IconName });
			}
		}
		if (samplePage) {
			sections.push({ id: 'indicators', label: 'Indicators', icon: 'lookup' as IconName });
			sections.push({ id: 'mitre', label: 'MITRE ATT&CK', icon: 'system' as IconName });
		}
		return sections;
	});

	let activeSectionId = $state('overview');

	// A section counts as "current" once its top scrolls above this line (px from
	// the top of the scroll area), leaving room for the section heading.
	const SPY_OFFSET = 130;

	// While a click-triggered smooth scroll is in flight we pin the highlight to the
	// clicked target instead of letting it walk through every section we pass.
	let programmatic = false;
	let programmaticTarget: string | null = null;
	let programmaticTimer: ReturnType<typeof setTimeout> | undefined;

	// The report content scrolls inside <main> (overflow-auto), not the window, so
	// resolve the nearest scrollable ancestor rather than assuming the document.
	function findScroller(el: HTMLElement): HTMLElement {
		let node: HTMLElement | null = el.parentElement;
		while (node) {
			const overflowY = getComputedStyle(node).overflowY;
			if (
				(overflowY === 'auto' || overflowY === 'scroll') &&
				node.scrollHeight > node.clientHeight
			) {
				return node;
			}
			node = node.parentElement;
		}
		return (document.scrollingElement as HTMLElement | null) ?? document.documentElement;
	}

	function scrollToSection(id: string) {
		const el = document.getElementById(`section-${id}`);
		if (!el) return;
		activeSectionId = id;
		programmatic = true;
		programmaticTarget = id;
		clearTimeout(programmaticTimer);
		programmaticTimer = setTimeout(() => {
			programmatic = false;
			programmaticTarget = null;
		}, 1200);
		el.scrollIntoView({ behavior: 'smooth', block: 'start' });
	}

	// Reset to the first section when navigating to a different sample.
	let lastSha: string | null = null;
	$effect(() => {
		const sha = samplePage?.sha256 ?? null;
		if (sha !== lastSha) {
			lastSha = sha;
			activeSectionId = 'overview';
		}
	});

	// Scroll-spy: highlight whichever section currently sits near the top of the
	// scroll area. Re-evaluated on scroll (rAF-throttled) and when the list changes.
	$effect(() => {
		if (!samplePage) return;
		const order = dynamicSections.map((s) => s.id);
		if (order.length === 0) return;

		let scroller: HTMLElement | null = null;
		let queued = false;

		function settle(current: string) {
			if (programmatic) {
				// Stay pinned to the click target until we actually reach it.
				if (current === programmaticTarget) {
					programmatic = false;
					programmaticTarget = null;
					clearTimeout(programmaticTimer);
				}
				return;
			}
			activeSectionId = current;
		}

		function recompute() {
			queued = false;
			const els = order
				.map((id) => ({ id, el: document.getElementById(`section-${id}`) }))
				.filter((s): s is { id: string; el: HTMLElement } => s.el !== null);
			if (els.length === 0) return;

			scroller ??= findScroller(els[0].el);

			// At the bottom the last section may be too short to reach the line, so
			// snap to it once the scroll area can't go any further.
			if (scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 4) {
				settle(els[els.length - 1].id);
				return;
			}

			let current = els[0].id;
			let bestTop = -Infinity;
			for (const { id, el } of els) {
				const top = el.getBoundingClientRect().top;
				if (top <= SPY_OFFSET && top > bestTop) {
					bestTop = top;
					current = id;
				}
			}
			settle(current);
		}

		function onScroll() {
			if (queued) return;
			queued = true;
			requestAnimationFrame(recompute);
		}

		const raf = requestAnimationFrame(recompute);
		// Capture phase so this one window-level listener also catches scroll events
		// from the inner <main> scroller (scroll doesn't bubble).
		window.addEventListener('scroll', onScroll, { passive: true, capture: true });

		return () => {
			cancelAnimationFrame(raf);
			window.removeEventListener('scroll', onScroll, true);
		};
	});
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

		<!-- Report sections (when on sample page) -->
		{#if samplePage}
			{#if !sidebar.collapsed}
				<div class="text-xs font-medium text-[var(--color-text-secondary)] px-3 mt-6 mb-3">
					Report
				</div>
			{:else}
				<div class="my-3 mx-2 border-t border-[var(--color-border)]"></div>
			{/if}
			<div class="space-y-2">
				{#each dynamicSections as item (item.id)}
					<button
						type="button"
						onclick={() => scrollToSection(item.id)}
						class="flex items-center gap-3 px-3 py-2 rounded-lg text-left w-full transition-colors
							{sidebar.collapsed ? 'justify-center px-0' : ''}
							{activeSectionId === item.id ? 'text-[#F4F4FF]' : 'text-[#8A8F94] hover:text-[#F4F4FF]'}"
						title={sidebar.collapsed ? item.label : undefined}
					>
						<div
							class="w-10 h-10 shrink-0 rounded-xl flex items-center justify-center transition-colors
								{activeSectionId === item.id ? 'bg-[#1D2342] text-[#516CF9]' : 'bg-[#25272C] text-[#8A8F94]'}"
						>
							<Icon path={icons[item.icon]} />
						</div>
						{#if !sidebar.collapsed}
							<span class="text-sm font-medium truncate">{item.label}</span>
						{/if}
					</button>
				{/each}
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
