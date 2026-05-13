<script lang="ts">
	interface Props {
		bytes_b64: string;
		offset?: number;
	}
	let { bytes_b64, offset = 0 }: Props = $props();

	function decode(b64: string): Uint8Array {
		try {
			const bin = atob(b64);
			const out = new Uint8Array(bin.length);
			for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
			return out;
		} catch {
			return new Uint8Array();
		}
	}

	const bytes = $derived(decode(bytes_b64));

	function hex2(n: number): string {
		return n.toString(16).padStart(2, '0');
	}
	function offsetHex(n: number): string {
		return n.toString(16).padStart(8, '0');
	}

	const rows = $derived.by(() => {
		const out: { off: string; hex: string; ascii: string }[] = [];
		for (let i = 0; i < bytes.length; i += 16) {
			const slice = bytes.slice(i, i + 16);
			const hexParts: string[] = [];
			let ascii = '';
			for (let j = 0; j < slice.length; j++) {
				hexParts.push(hex2(slice[j]));
				const c = slice[j];
				ascii += c >= 32 && c < 127 ? String.fromCharCode(c) : '.';
			}
			while (hexParts.length < 16) hexParts.push('  ');
			out.push({
				off: offsetHex(offset + i),
				hex: hexParts.slice(0, 8).join(' ') + '  ' + hexParts.slice(8).join(' '),
				ascii
			});
		}
		return out;
	});
</script>

<div class="overflow-hidden rounded-lg bg-[var(--color-bg-primary)]">
	<div class="px-4 py-2">
		<span class="font-mono text-[10px] uppercase text-[var(--color-text-secondary)]">
			hex · {bytes.length} bytes
		</span>
	</div>
	<pre
		class="overflow-x-auto border-t border-[var(--color-border)]/30 px-4 py-3 font-mono text-xs leading-5 text-[var(--color-text-primary)]">{#each rows as r, i (i)}<span
				class="select-none text-[var(--color-text-secondary)]">{r.off}</span
			>  {r.hex}  <span class="text-[var(--color-accent)]/60">{r.ascii}</span>
		{/each}</pre>
</div>
