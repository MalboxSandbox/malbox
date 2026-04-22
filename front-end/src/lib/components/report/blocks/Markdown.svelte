<script lang="ts">
	interface Props {
		text: string;
	}
	let { text }: Props = $props();

	function escape(s: string): string {
		return s
			.replaceAll('&', '&amp;')
			.replaceAll('<', '&lt;')
			.replaceAll('>', '&gt;')
			.replaceAll('"', '&quot;')
			.replaceAll("'", '&#39;');
	}

	function renderInline(s: string): string {
		let out = escape(s);
		// [text](url) — url restricted to http(s) and mailto
		out = out.replace(
			/\[([^\]]+)\]\((https?:\/\/[^\s)]+|mailto:[^\s)]+)\)/g,
			(_, t: string, u: string) =>
				`<a href="${u}" target="_blank" rel="noopener" class="text-[var(--color-accent)] hover:underline">${t}</a>`
		);
		// `code`
		out = out.replace(
			/`([^`]+)`/g,
			(_, c: string) =>
				`<code class="rounded bg-[var(--color-bg-tertiary)] px-1 py-0.5 font-mono text-xs">${c}</code>`
		);
		// **bold**
		out = out.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
		// *italic*
		out = out.replace(/(^|\s)\*([^*]+)\*(?=\s|$)/g, '$1<em>$2</em>');
		return out;
	}

	function renderBlock(raw: string): string {
		const lines = raw.replaceAll('\r\n', '\n').split('\n');
		const parts: string[] = [];
		let i = 0;
		while (i < lines.length) {
			const line = lines[i];
			if (line.trim() === '') {
				i++;
				continue;
			}
			// unordered list
			if (/^\s*[-*]\s+/.test(line)) {
				const items: string[] = [];
				while (i < lines.length && /^\s*[-*]\s+/.test(lines[i])) {
					items.push(`<li>${renderInline(lines[i].replace(/^\s*[-*]\s+/, ''))}</li>`);
					i++;
				}
				parts.push(`<ul class="list-disc space-y-1 pl-6">${items.join('')}</ul>`);
				continue;
			}
			// ordered list
			if (/^\s*\d+\.\s+/.test(line)) {
				const items: string[] = [];
				while (i < lines.length && /^\s*\d+\.\s+/.test(lines[i])) {
					items.push(`<li>${renderInline(lines[i].replace(/^\s*\d+\.\s+/, ''))}</li>`);
					i++;
				}
				parts.push(`<ol class="list-decimal space-y-1 pl-6">${items.join('')}</ol>`);
				continue;
			}
			// paragraph: consume consecutive non-empty non-list lines
			const para: string[] = [];
			while (
				i < lines.length &&
				lines[i].trim() !== '' &&
				!/^\s*[-*]\s+/.test(lines[i]) &&
				!/^\s*\d+\.\s+/.test(lines[i])
			) {
				para.push(renderInline(lines[i]));
				i++;
			}
			parts.push(`<p>${para.join('<br />')}</p>`);
		}
		return parts.join('');
	}

	const html = $derived(renderBlock(text));
</script>

<div class="prose prose-invert max-w-none space-y-3 text-sm text-[var(--color-text-primary)]">
	{@html html}
</div>
