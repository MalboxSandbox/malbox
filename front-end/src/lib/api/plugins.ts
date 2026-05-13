import { requestJson, type FetchLike } from './client';
import type { Plugin, PluginType, AvailablePlugins, Platform } from './types';

export async function listPlugins(fetchFn: FetchLike, type?: PluginType): Promise<Plugin[]> {
	const path = type ? `/v1/plugins?type=${encodeURIComponent(type)}` : '/v1/plugins';
	return requestJson<Plugin[]>(fetchFn, path);
}

export async function listAvailablePlugins(
	fetchFn: FetchLike,
	platform?: Platform
): Promise<AvailablePlugins> {
	const path = platform
		? `/v1/plugins/available?platform=${encodeURIComponent(platform)}`
		: '/v1/plugins/available';
	return requestJson<AvailablePlugins>(fetchFn, path);
}
