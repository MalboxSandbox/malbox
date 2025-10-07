export type PluginType = 'plugin' | 'module';

export interface MarketplaceItem {
	id: string;
	name: string;
	type: PluginType;
	author: string;
	description: string;
	rating: number; // 0-5
	avatarColor: 'blue' | 'red';
	isOfficial: boolean;
}

export type MarketplaceFilter = 'all' | 'modules' | 'plugins';
