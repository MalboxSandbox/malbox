import { env } from '$env/dynamic/private';

export function getBackendUrl(): string {
	return env.MALBOX_API_URL ?? 'http://127.0.0.1:8080';
}
