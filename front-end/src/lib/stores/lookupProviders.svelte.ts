import type { LookupProvider } from '$lib/components/context-menu/types';

const STORAGE_KEY = 'malbox:lookupProviders';

// TODO: Provider configuration is stored in localStorage as a temporary measure.
// Migrate to backend/DB persistence so providers are shared across sessions and users.

export function faviconUrl(urlTemplate: string): string | undefined {
	try {
		// eslint-disable-next-line svelte/prefer-svelte-reactivity -- non-reactive utility function
		const domain = new URL(urlTemplate.replace('{value}', 'x')).hostname;
		return `https://www.google.com/s2/favicons?domain=${domain}&sz=32`;
	} catch {
		return undefined;
	}
}

const DEFAULT_PROVIDERS: LookupProvider[] = [
	{
		id: 'virustotal',
		name: 'VirusTotal',
		urlTemplate: 'https://www.virustotal.com/gui/search/{value}',
		indicatorTypes: ['sha256', 'sha1', 'md5', 'ip', 'domain', 'url'],
		favicon: faviconUrl('https://www.virustotal.com/'),
		enabled: true
	},
	{
		id: 'malwarebazaar',
		name: 'MalwareBazaar',
		urlTemplate: 'https://bazaar.abuse.ch/sample/{value}/',
		indicatorTypes: ['sha256', 'md5'],
		favicon: faviconUrl('https://bazaar.abuse.ch/'),
		enabled: true
	},
	{
		id: 'hybrid-analysis',
		name: 'Hybrid Analysis',
		urlTemplate: 'https://www.hybrid-analysis.com/search?query={value}',
		indicatorTypes: ['sha256', 'sha1', 'md5'],
		favicon: faviconUrl('https://www.hybrid-analysis.com/'),
		enabled: true
	},
	{
		id: 'abuseipdb',
		name: 'AbuseIPDB',
		urlTemplate: 'https://www.abuseipdb.com/check/{value}',
		indicatorTypes: ['ip'],
		favicon: faviconUrl('https://www.abuseipdb.com/'),
		enabled: true
	},
	{
		id: 'shodan',
		name: 'Shodan',
		urlTemplate: 'https://www.shodan.io/host/{value}',
		indicatorTypes: ['ip'],
		favicon: faviconUrl('https://www.shodan.io/'),
		enabled: true
	},
	{
		id: 'urlhaus',
		name: 'URLhaus',
		urlTemplate: 'https://urlhaus.abuse.ch/browse.php?search={value}',
		indicatorTypes: ['url', 'domain'],
		favicon: faviconUrl('https://urlhaus.abuse.ch/'),
		enabled: true
	},
	{
		id: 'urlscan',
		name: 'URLScan.io',
		urlTemplate: 'https://urlscan.io/search/#{value}',
		indicatorTypes: ['url'],
		favicon: faviconUrl('https://urlscan.io/'),
		enabled: true
	}
];

function loadFromStorage(): LookupProvider[] {
	if (typeof window === 'undefined') return DEFAULT_PROVIDERS;
	const raw = localStorage.getItem(STORAGE_KEY);
	if (!raw) return DEFAULT_PROVIDERS;
	try {
		return JSON.parse(raw) as LookupProvider[];
	} catch {
		return DEFAULT_PROVIDERS;
	}
}

function saveToStorage(providers: LookupProvider[]): void {
	if (typeof window === 'undefined') return;
	localStorage.setItem(STORAGE_KEY, JSON.stringify(providers));
}

class LookupProviderStore {
	private items = $state<LookupProvider[]>(loadFromStorage());

	get providers(): LookupProvider[] {
		return this.items;
	}

	getForType(subtype: string): LookupProvider[] {
		return this.items.filter((p) => p.enabled && p.indicatorTypes.includes(subtype));
	}

	add(provider: Omit<LookupProvider, 'id'>): void {
		const favicon = provider.favicon ?? faviconUrl(provider.urlTemplate);
		const newProvider = { ...provider, id: crypto.randomUUID(), favicon };
		this.items = [...this.items, newProvider];
		saveToStorage(this.items);
	}

	update(id: string, changes: Partial<Omit<LookupProvider, 'id'>>): void {
		this.items = this.items.map((p) => {
			if (p.id !== id) return p;
			const updated = { ...p, ...changes };
			if (changes.urlTemplate && !changes.favicon) {
				updated.favicon = faviconUrl(changes.urlTemplate);
			}
			return updated;
		});
		saveToStorage(this.items);
	}

	remove(id: string): void {
		this.items = this.items.filter((p) => p.id !== id);
		saveToStorage(this.items);
	}

	reset(): void {
		this.items = DEFAULT_PROVIDERS;
		saveToStorage(this.items);
	}
}

export const lookupProviders = new LookupProviderStore();
