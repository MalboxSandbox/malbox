import { requestJson, type FetchLike } from './client';
import type { Plugin, PluginType } from './types';

export async function listPlugins(fetchFn: FetchLike, type?: PluginType): Promise<Plugin[]> {
	const path = type ? `/api/plugins?type=${encodeURIComponent(type)}` : '/api/plugins';
	return requestJson<Plugin[]>(fetchFn, path);
}
