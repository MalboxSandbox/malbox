// Malbox ships as a single-page app served by the Rust daemon (same-origin
// `/v1` API). Disable SSR so `adapter-static` emits a client-rendered shell
// with a fallback document; all data loading happens in the browser against
// the daemon's API.
export const ssr = false;
export const prerender = false;
