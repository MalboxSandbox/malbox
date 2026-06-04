/// <reference types="vitest/config" />
import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
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
