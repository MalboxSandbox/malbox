<script lang="ts">
	import type { Ttp } from '$lib/api/types';

	interface Props {
		ttps: Ttp[];
	}

	let { ttps }: Props = $props();

	const TACTIC_PATTERNS: [RegExp, string][] = [
		[/^TA00(0[1-9]|1[0-4])/, ''],
		[/^T1595|^T1592|^T1589|^T1590|^T1591|^T1598|^T1597|^T1596/, 'Reconnaissance'],
		[/^T1583|^T1586|^T1584|^T1587|^T1585|^T1588|^T1608/, 'Resource Development'],
		[/^T1189|^T1190|^T1133|^T1200|^T1566|^T1091|^T1195|^T1199|^T1078/, 'Initial Access'],
		[/^T1059|^T1203|^T1559|^T1106|^T1053|^T1129|^T1072|^T1569|^T1204|^T1047/, 'Execution'],
		[/^T1098|^T1197|^T1547|^T1037|^T1136|^T1543|^T1546|^T1574/, 'Persistence'],
		[/^T1548|^T1134|^T1484|^T1611/, 'Privilege Escalation'],
		[
			/^T1140|^T1006|^T1480|^T1211|^T1222|^T1564|^T1070|^T1202|^T1036|^T1556|^T1578|^T1562|^T1027|^T1542|^T1055|^T1207|^T1014|^T1218|^T1216|^T1221|^T1205|^T1127|^T1535|^T1550|^T1497/,
			'Defense Evasion'
		],
		[
			/^T1557|^T1110|^T1555|^T1212|^T1187|^T1606|^T1056|^T1556|^T1111|^T1621|^T1040|^T1003|^T1528|^T1558|^T1539|^T1552/,
			'Credential Access'
		],
		[
			/^T1087|^T1010|^T1217|^T1580|^T1538|^T1526|^T1619|^T1613|^T1622|^T1482|^T1083|^T1046|^T1135|^T1040|^T1120|^T1069|^T1057|^T1012|^T1018|^T1518|^T1082|^T1016|^T1049|^T1033|^T1007/,
			'Discovery'
		],
		[/^T1210|^T1534|^T1570|^T1563|^T1021|^T1091|^T1072|^T1080|^T1550/, 'Lateral Movement'],
		[
			/^T1560|^T1123|^T1119|^T1185|^T1115|^T1530|^T1602|^T1213|^T1005|^T1039|^T1025|^T1074|^T1113|^T1125/,
			'Collection'
		],
		[
			/^T1071|^T1132|^T1001|^T1568|^T1573|^T1008|^T1104|^T1095|^T1571|^T1572|^T1090|^T1219|^T1102/,
			'Command and Control'
		],
		[/^T1020|^T1030|^T1048|^T1041|^T1011|^T1052|^T1567|^T1029|^T1537/, 'Exfiltration'],
		[
			/^T1531|^T1485|^T1486|^T1565|^T1491|^T1561|^T1499|^T1495|^T1490|^T1498|^T1496|^T1489|^T1529/,
			'Impact'
		]
	];

	function guessTactic(id: string): string {
		for (const [pattern, tactic] of TACTIC_PATTERNS) {
			if (tactic && pattern.test(id)) return tactic;
		}
		return 'Other';
	}

	const grouped = $derived.by(() => {
		// eslint-disable-next-line svelte/prefer-svelte-reactivity -- local computation variable
		const map = new Map<string, Ttp[]>();
		for (const ttp of ttps) {
			const tactic = guessTactic(ttp.id);
			const existing = map.get(tactic);
			if (existing) {
				existing.push(ttp);
			} else {
				map.set(tactic, [ttp]);
			}
		}
		return Array.from(map.entries()).map(([tactic, items]) => ({ tactic, items }));
	});
</script>

{#if ttps.length > 0}
	<div class="flex flex-col gap-2">
		{#each grouped as group (group.tactic)}
			<div class="flex items-start gap-4">
				<span
					class="min-w-[120px] shrink-0 pt-1 text-right text-[10px] font-medium uppercase tracking-wide text-[var(--color-text-secondary)]"
				>
					{group.tactic}
				</span>
				<div class="flex flex-wrap gap-1.5">
					{#each group.items as ttp (ttp.id)}
						<a
							href="https://attack.mitre.org/techniques/{ttp.id.split('.')[0]}/"
							target="_blank"
							rel="noopener"
							class="rounded-md bg-[var(--color-bg-card)] px-2.5 py-1 text-[10px] text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-bg-tertiary)] hover:text-[var(--color-text-primary)]"
						>
							{ttp.id} &middot; {ttp.name}
						</a>
					{/each}
				</div>
			</div>
		{/each}
	</div>
{/if}
