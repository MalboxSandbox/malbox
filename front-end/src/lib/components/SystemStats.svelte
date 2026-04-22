<script lang="ts">
	import type { Machine, MachineStatus } from '$lib/api/types';

	interface Props {
		machines: Machine[];
	}

	let { machines }: Props = $props();

	function countByStatus(predicate: (status: MachineStatus) => boolean): number {
		return machines.filter((m) => predicate(m.status)).length;
	}

	const total = $derived(machines.length);
	const available = $derived(countByStatus((s) => s === 'ready'));
	const busy = $derived(
		countByStatus((s) => s === 'assigned' || s === 'provisioning' || s === 'reverting')
	);
	const other = $derived(total - available - busy);

	function pct(n: number): number {
		return total === 0 ? 0 : Math.round((n / total) * 100);
	}
</script>

<div class="bg-[var(--color-bg-secondary)] rounded-2xl p-6">
	<h2
		class="text-xl font-semibold text-[var(--color-text-primary)] mb-6 pb-6 border-b border-[var(--color-border)]"
	>
		System Statistics
	</h2>

	<div class="space-y-10">
		<div class="flex items-center gap-2">
			<span class="text-[var(--color-text-secondary)]">Registered machines:</span>
			<span class="text-[var(--color-text-primary)]">{total}</span>
		</div>

		<div>
			<div class="flex justify-between mb-2">
				<span class="text-[var(--color-text-secondary)]">Available:</span>
				<span class="text-[var(--color-text-secondary)]">{available} of {total}</span>
			</div>
			<div class="w-full h-2 bg-[var(--color-bg-tertiary)] rounded-full overflow-hidden">
				<div
					class="h-full bg-[var(--color-accent)] rounded-full"
					style="width: {pct(available)}%"
				></div>
			</div>
		</div>

		<div>
			<div class="flex justify-between mb-2">
				<span class="text-[var(--color-text-secondary)]">Busy:</span>
				<span class="text-[var(--color-text-secondary)]">{busy} of {total}</span>
			</div>
			<div class="w-full h-2 bg-[var(--color-bg-tertiary)] rounded-full overflow-hidden">
				<div class="h-full bg-[var(--color-accent)] rounded-full" style="width: {pct(busy)}%"></div>
			</div>
		</div>

		<div>
			<div class="flex justify-between mb-2">
				<span class="text-[var(--color-text-secondary)]">Other / offline:</span>
				<span class="text-[var(--color-text-secondary)]">{other} of {total}</span>
			</div>
			<div class="w-full h-2 bg-[var(--color-bg-tertiary)] rounded-full overflow-hidden">
				<div
					class="h-full bg-[var(--color-accent)] rounded-full"
					style="width: {pct(other)}%"
				></div>
			</div>
		</div>
	</div>
</div>
