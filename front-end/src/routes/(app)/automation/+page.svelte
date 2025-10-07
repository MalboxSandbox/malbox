<script lang="ts">
	import ApiKeyHeader from '$lib/components/ApiKeyHeader.svelte';
	import ActivityChart from '$lib/components/ActivityChart.svelte';
	import StatisticsChart from '$lib/components/StatisticsChart.svelte';
	import RecentSubmissions from '$lib/components/RecentSubmissions.svelte';
	import type { ActivityData, TimeRange, Statistics, Submission } from '$lib/types/automation';

	// Mock data - replace with actual API calls
	let selectedKey = $state<string | null>('No keys selected');
	let timeRange = $state<TimeRange>('week');

	const weekData: ActivityData[] = [
		{ day: 'Mon', value: 40 },
		{ day: 'Tue', value: 48 },
		{ day: 'Wed', value: 30 },
		{ day: 'Thu', value: 75 },
		{ day: 'Fri', value: 50 },
		{ day: 'Sat', value: 130 },
		{ day: 'Sun', value: 140 }
	];

	let activityData = $state<ActivityData[]>(weekData);

	const stats: Statistics = {
		totalSubmissions: 1933,
		clean: 1450, // 75%
		suspicious: 290, // 15%
		malicious: 193 // 10%
	};

	const submissions: Submission[] = [
		{
			id: '1',
			date: '2024-04-06',
			time: '14:45',
			type: 'url',
			value: 'https://suspicious-site.com/malware',
			status: 'finished',
			progress: 3,
			total: 10,
			apiKey: 'Project Perso 1'
		},
		{
			id: '2',
			date: '2024-04-06',
			time: '14:45',
			type: 'file',
			value: 'malware_sample.exe',
			status: 'finished',
			progress: 3,
			total: 10,
			apiKey: 'Project Perso 1'
		},
		{
			id: '3',
			date: '2024-04-06',
			time: '14:45',
			type: 'hash',
			value: 'a3f5e8d2c1b4f6e9a2d5c8b7f4e1d3c6',
			status: 'pending',
			progress: 0,
			total: 10,
			apiKey: 'Project Perso 1'
		},
		{
			id: '4',
			date: '2024-04-06',
			time: '14:45',
			type: 'file',
			value: 'suspicious_document.pdf',
			status: 'pending',
			progress: 0,
			total: 10,
			apiKey: 'Project Perso 1'
		}
	];

	function handleManageKeys() {
		// Navigate to API keys management page
		console.log('Manage API keys');
	}

	function handleTimeRangeChange(range: TimeRange) {
		timeRange = range;
		// Fetch new data based on time range
		// For now, just use mock data
	}
</script>

<div class="max-w-7xl mx-auto">
	<ApiKeyHeader {selectedKey} onManageKeys={handleManageKeys} />

	<div class="grid grid-cols-2 gap-6 mb-6">
		<ActivityChart data={activityData} {timeRange} onTimeRangeChange={handleTimeRangeChange} />
		<StatisticsChart {stats} />
	</div>

	<RecentSubmissions {submissions} />
</div>
