<script lang="ts">
	import { calloutClasses } from '../styles';
	import type { CalloutLevel } from '$lib/api/types';

	interface Props {
		level: CalloutLevel;
		text: string;
	}
	let { level, text }: Props = $props();
	const cls = $derived(calloutClasses(level));
</script>

<div class="rounded-lg border px-4 py-3 text-sm {cls}">
	{text}
</div>
