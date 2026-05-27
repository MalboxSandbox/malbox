import { getRecipe } from '$lib/api/recipes';
import { error } from '@sveltejs/kit';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, params, depends }) => {
	depends('malbox:recipes');
	try {
		const recipe = await getRecipe(fetch, params.id);
		return { recipe };
	} catch {
		error(404, 'Recipe not found');
	}
};
