import { listRecipes } from '#lib/api/recipes.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, depends }) => {
	depends('malbox:recipes');
	let recipes: Awaited<ReturnType<typeof listRecipes>> = [];
	try {
		recipes = await listRecipes(fetch);
	} catch {
		// recipes API may not be available
	}
	return { recipes };
};
