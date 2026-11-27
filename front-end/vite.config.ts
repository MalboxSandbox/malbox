/// <reference types="vitest/config" />
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit({
			// Consult https://svelte.dev/docs/kit/integrations
			// for more information about preprocessors
			preprocess: vitePreprocess(),

			// Single-page app: `adapter-static` with a fallback document emits a
			// client-rendered bundle (`build/`) that the Rust daemon serves directly.
			// SSR is disabled in the root +layout.ts; routing happens in the browser.
			adapter: adapter({ fallback: 'index.html' })
		})
	],
	server: {
		// In dev the SPA is served by Vite; forward same-origin `/v1` API calls
		// to the running malbox daemon (mirrors how the daemon serves both in
		// production).
		proxy: {
			'/v1': 'http://127.0.0.1:8080'
		}
	},
	test: {
		include: ['src/**/*.test.ts']
	}
});
