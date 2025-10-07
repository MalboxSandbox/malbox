
// this file is generated — do not edit it


declare module "svelte/elements" {
	export interface HTMLAttributes<T> {
		'data-sveltekit-keepfocus'?: true | '' | 'off' | undefined | null;
		'data-sveltekit-noscroll'?: true | '' | 'off' | undefined | null;
		'data-sveltekit-preload-code'?:
			| true
			| ''
			| 'eager'
			| 'viewport'
			| 'hover'
			| 'tap'
			| 'off'
			| undefined
			| null;
		'data-sveltekit-preload-data'?: true | '' | 'hover' | 'tap' | 'off' | undefined | null;
		'data-sveltekit-reload'?: true | '' | 'off' | undefined | null;
		'data-sveltekit-replacestate'?: true | '' | 'off' | undefined | null;
	}
}

export {};


declare module "$app/types" {
	export interface AppTypes {
		RouteId(): "/(app)" | "/" | "/auth" | "/auth/login" | "/auth/register" | "/(app)/automation" | "/(app)/dashboard" | "/(app)/marketplace" | "/(app)/marketplace/[id]" | "/(app)/submissions";
		RouteParams(): {
			"/(app)/marketplace/[id]": { id: string }
		};
		LayoutParams(): {
			"/(app)": { id?: string };
			"/": { id?: string };
			"/auth": Record<string, never>;
			"/auth/login": Record<string, never>;
			"/auth/register": Record<string, never>;
			"/(app)/automation": Record<string, never>;
			"/(app)/dashboard": Record<string, never>;
			"/(app)/marketplace": { id?: string };
			"/(app)/marketplace/[id]": { id: string };
			"/(app)/submissions": Record<string, never>
		};
		Pathname(): "/" | "/auth" | "/auth/" | "/auth/login" | "/auth/login/" | "/auth/register" | "/auth/register/" | "/automation" | "/automation/" | "/dashboard" | "/dashboard/" | "/marketplace" | "/marketplace/" | `/marketplace/${string}` & {} | `/marketplace/${string}/` & {} | "/submissions" | "/submissions/";
		ResolvedPathname(): `${"" | `/${string}`}${ReturnType<AppTypes['Pathname']>}`;
		Asset(): "/robots.txt" | string & {};
	}
}