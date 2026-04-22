<script lang="ts">
	import { classificationClasses } from './styles';
	import type { Classification } from '$lib/api/types';

	interface Props {
		score: number;
		classification?: Classification | string;
	}

	let { score, classification }: Props = $props();
	const cls = $derived(classificationClasses(classification));
	const clamped = $derived(Math.max(0, Math.min(100, score)));
</script>

<div class="flex items-center gap-3">
	<div class="h-2 flex-1 overflow-hidden rounded-full bg-[var(--color-bg-tertiary)]">
		<div class="h-full rounded-full {cls.dot}" style="width: {clamped}%"></div>
	</div>
	<span class="font-mono text-xs text-[var(--color-text-secondary)]">{clamped}/100</span>
</div>
