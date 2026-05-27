import { requestJson, type FetchLike } from './client';
import type { Recipe, RecipeScope, RecipeStep } from './types';

export async function listRecipes(
	fetchFn: FetchLike,
	filters?: { author?: string; tag?: string; scope?: RecipeScope }
): Promise<Recipe[]> {
	const params = new URLSearchParams();
	if (filters?.author) params.set('author', filters.author);
	if (filters?.tag) params.set('tag', filters.tag);
	if (filters?.scope) params.set('scope', filters.scope);
	const qs = params.toString();
	return requestJson<Recipe[]>(fetchFn, `/v1/recipes${qs ? `?${qs}` : ''}`);
}

export async function getRecipe(fetchFn: FetchLike, id: string): Promise<Recipe> {
	return requestJson<Recipe>(fetchFn, `/v1/recipes/${id}`);
}

export async function createRecipe(
	fetchFn: FetchLike,
	recipe: {
		name: string;
		description?: string;
		author: string;
		scope?: RecipeScope;
		tags?: string[];
		steps: RecipeStep[];
	}
): Promise<Recipe> {
	return requestJson<Recipe>(fetchFn, '/v1/recipes', {
		method: 'POST',
		body: JSON.stringify(recipe),
		headers: { 'Content-Type': 'application/json' }
	});
}

export async function updateRecipe(
	fetchFn: FetchLike,
	id: string,
	recipe: {
		name: string;
		description?: string;
		scope?: RecipeScope;
		tags?: string[];
		steps: RecipeStep[];
	}
): Promise<Recipe> {
	return requestJson<Recipe>(fetchFn, `/v1/recipes/${id}`, {
		method: 'PUT',
		body: JSON.stringify(recipe),
		headers: { 'Content-Type': 'application/json' }
	});
}

export async function deleteRecipe(fetchFn: FetchLike, id: string): Promise<void> {
	await requestJson(fetchFn, `/v1/recipes/${id}`, { method: 'DELETE' });
}
