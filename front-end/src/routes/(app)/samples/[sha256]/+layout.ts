import { error } from '@sveltejs/kit';
import { getSampleOverview } from '$lib/api/samples';
import { isApiError } from '$lib/api/errors';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ fetch, params, depends }) => {
	depends('malbox:sample');
	const sha256 = params.sha256;

	try {
		const overview = await getSampleOverview(fetch, sha256);
		return { overview, sha256 };
	} catch (err) {
		if (isApiError(err) && err.status === 404) {
			throw error(404, `Sample not found`);
		}
		throw err;
	}
};
