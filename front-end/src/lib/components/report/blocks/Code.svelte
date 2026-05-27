<script lang="ts">
	import CodeViewer from '$lib/components/CodeViewer.svelte';
	import { contextmenu } from '$lib/actions/contextmenu';

	interface Props {
		language: string;
		text: string;
	}
	let { language, text }: Props = $props();
</script>

<div use:contextmenu={{ type: 'code-block', value: '', selectionOnly: true }}>
	<CodeViewer code={text} {language} />
</div>
