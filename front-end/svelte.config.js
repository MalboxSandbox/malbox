import { preprocessMeltUI, sequence } from '@melt-ui/pp';
import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
/** @type {import('@sveltejs/kit').Config}*/
const config = {
	// Consult https://svelte.dev/docs/kit/integrations
	// for more information about preprocessors
	preprocess: sequence([vitePreprocess(), preprocessMeltUI()]),
	kit: {
		// Single-page app: `adapter-static` with a fallback document emits a
		// client-rendered bundle (`build/`) that the Rust daemon serves directly.
		// SSR is disabled in the root +layout.ts; routing happens in the browser.
		adapter: adapter({
			fallback: 'index.html'
		})
	}
};
export default config;
