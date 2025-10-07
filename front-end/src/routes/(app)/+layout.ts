import { redirect } from '@sveltejs/kit';
import { browser } from '$app/environment';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async () => {
	// Check localStorage for authentication
	if (browser) {
		const isAuthenticated = localStorage.getItem('mockAuth') === 'true';

		if (!isAuthenticated) {
			throw redirect(303, '/auth/login');
		}
	}

	return {};
};
