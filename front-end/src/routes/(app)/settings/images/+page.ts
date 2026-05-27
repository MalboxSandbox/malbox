import { listImages } from '$lib/api/images';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, depends }) => {
	depends('malbox:images');
	const images = await listImages(fetch);
	return { images };
};
