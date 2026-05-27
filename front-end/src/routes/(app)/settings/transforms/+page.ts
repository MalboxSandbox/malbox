import { listCustomTransforms } from '$lib/api/transforms';
import { listRecipes } from '$lib/api/recipes';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, depends }) => {
	depends('malbox:transforms');
	depends('malbox:recipes');
	let transforms: Awaited<ReturnType<typeof listCustomTransforms>> = [];
	let recipes: Awaited<ReturnType<typeof listRecipes>> = [];
	try {
		transforms = await listCustomTransforms(fetch);
	} catch {
		// API may not be available
	}
	try {
		recipes = await listRecipes(fetch);
	} catch {
		// API may not be available
	}
	return { transforms, recipes };
};
