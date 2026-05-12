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
			// table
			if (/^\s*\|(.+)\|/.test(line)) {
				const tableLines: string[] = [];
				while (i < lines.length && /^\s*\|(.+)\|/.test(lines[i])) {
					tableLines.push(lines[i]);
					i++;
				}
				if (tableLines.length >= 2 && /^\s*\|[\s:]*-+/.test(tableLines[1])) {
					const parseCells = (row: string) =>
						row
							.trim()
							.replace(/^\||\|$/g, '')
							.split('|')
							.map((c) => c.trim());
					const headers = parseCells(tableLines[0]);
					const aligns = parseCells(tableLines[1]).map((c) => {
						if (c.startsWith(':') && c.endsWith(':')) return 'center';
						if (c.endsWith(':')) return 'right';
						return 'left';
					});
					const headerHtml = headers
						.map(
							(h, idx) =>
								`<th class="px-3 py-2 text-left text-xs font-medium text-[var(--color-text-secondary)]" style="text-align:${aligns[idx] ?? 'left'}">${renderInline(h)}</th>`
						)
						.join('');
					const bodyRows = tableLines.slice(2);
					const rowsHtml = bodyRows
						.map((r) => {
							const cells = parseCells(r);
							return `<tr class="border-t border-[var(--color-border)]">${cells.map((c, idx) => `<td class="px-3 py-2 text-sm" style="text-align:${aligns[idx] ?? 'left'}">${renderInline(c)}</td>`).join('')}</tr>`;
						})
						.join('');
					parts.push(
						`<div class="overflow-x-auto"><table class="w-full border-collapse"><thead><tr class="border-b border-[var(--color-border)]">${headerHtml}</tr></thead><tbody>${rowsHtml}</tbody></table></div>`
					);
					continue;
				}
				// not a valid table, treat lines as paragraph
				parts.push(`<p>${tableLines.map((l) => renderInline(l)).join('<br />')}</p>`);
				continue;
			}
			// paragraph: consume consecutive non-empty non-list lines
			const para: string[] = [];
			while (
				i < lines.length &&
				lines[i].trim() !== '' &&
				!/^\s*[-*]\s+/.test(lines[i]) &&
				!/^\s*\d+\.\s+/.test(lines[i]) &&
				!/^\s*\|(.+)\|/.test(lines[i])
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
