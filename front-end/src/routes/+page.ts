import { auth } from '$lib/stores/auth.svelte';
import { redirect } from '@sveltejs/kit';
import { browser } from '$app/environment';

export function load() {
	if (browser) {
		// Check auth state from localStorage
		auth.checkAuth();

		if (!auth.isAuthenticated) {
			throw redirect(302, '/auth/login');
		} else {
			throw redirect(302, '/dashboard');
		}
	}

	return {};
}
