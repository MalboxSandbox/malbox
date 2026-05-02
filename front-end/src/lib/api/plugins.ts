import { requestJson, type FetchLike } from './client';
import type { Plugin, PluginType, AvailablePlugins, Platform } from './types';

export async function listPlugins(fetchFn: FetchLike, type?: PluginType): Promise<Plugin[]> {
	const path = type ? `/api/plugins?type=${encodeURIComponent(type)}` : '/api/plugins';
	return requestJson<Plugin[]>(fetchFn, path);
}

export async function listAvailablePlugins(
	fetchFn: FetchLike,
	platform?: Platform
): Promise<AvailablePlugins> {
	const path = platform
		? `/api/plugins/available?platform=${encodeURIComponent(platform)}`
		: '/api/plugins/available';
	return requestJson<AvailablePlugins>(fetchFn, path);
}
