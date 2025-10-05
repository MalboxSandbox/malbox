
// this file is generated — do not edit it


/// <reference types="@sveltejs/kit" />

/**
 * Environment variables [loaded by Vite](https://vitejs.dev/guide/env-and-mode.html#env-files) from `.env` files and `process.env`. Like [`$env/dynamic/private`](https://svelte.dev/docs/kit/$env-dynamic-private), this module cannot be imported into client-side code. This module only includes variables that _do not_ begin with [`config.kit.env.publicPrefix`](https://svelte.dev/docs/kit/configuration#env) _and do_ start with [`config.kit.env.privatePrefix`](https://svelte.dev/docs/kit/configuration#env) (if configured).
 * 
 * _Unlike_ [`$env/dynamic/private`](https://svelte.dev/docs/kit/$env-dynamic-private), the values exported from this module are statically injected into your bundle at build time, enabling optimisations like dead code elimination.
 * 
 * ```ts
 * import { API_KEY } from '$env/static/private';
 * ```
 * 
 * Note that all environment variables referenced in your code should be declared (for example in an `.env` file), even if they don't have a value until the app is deployed:
 * 
 * ```
 * MY_FEATURE_FLAG=""
 * ```
 * 
 * You can override `.env` values from the command line like so:
 * 
 * ```sh
 * MY_FEATURE_FLAG="enabled" npm run dev
 * ```
 */
declare module '$env/static/private' {
	export const SHELL: string;
	export const npm_command: string;
	export const WINDOWID: string;
	export const GHOSTTY_BIN_DIR: string;
	export const COLORTERM: string;
	export const _PYTHON_HOST_PLATFORM: string;
	export const hardeningDisable: string;
	export const TERM_PROGRAM_VERSION: string;
	export const configureFlags: string;
	export const DATABASE_URL: string;
	export const PC_CONFIG_FILES: string;
	export const mesonFlags: string;
	export const PKG_CONFIG_PATH: string;
	export const PYTHONNOUSERSITE: string;
	export const DEVENV_TASK_FILE: string;
	export const PGPORT: string;
	export const PYTHONHASHSEED: string;
	export const SSH_AUTH_SOCK: string;
	export const DIRENV_DIR: string;
	export const npm_config_verify_deps_before_run: string;
	export const STRINGS: string;
	export const LD_FOR_BUILD: string;
	export const NIX_CFLAGS_COMPILE_FOR_BUILD: string;
	export const DESKTOP_SESSION: string;
	export const SSH_AGENT_PID: string;
	export const DIRENV_FILE: string;
	export const EDITOR: string;
	export const XDG_SEAT: string;
	export const PWD: string;
	export const NIX_PROFILES: string;
	export const SOURCE_DATE_EPOCH: string;
	export const LOGNAME: string;
	export const XDG_SESSION_DESKTOP: string;
	export const XDG_SESSION_TYPE: string;
	export const NIX_ENFORCE_NO_NATIVE: string;
	export const AS_FOR_BUILD: string;
	export const CXX: string;
	export const XAUTHORITY: string;
	export const system: string;
	export const SIZE_FOR_BUILD: string;
	export const PC_SOCKET_PATH: string;
	export const _PYTHON_SYSCONFIGDATA_NAME: string;
	export const LIBCLANG_PATH: string;
	export const DEVENV_DOTFILE: string;
	export const WINDOWPATH: string;
	export const GDM_LANG: string;
	export const IN_NIX_SHELL: string;
	export const GHOSTTY_SHELL_FEATURES: string;
	export const HOME: string;
	export const USERNAME: string;
	export const NIX_BINTOOLS: string;
	export const LANG: string;
	export const STARSHIP_SHELL: string;
	export const cmakeFlags: string;
	export const CXX_FOR_BUILD: string;
	export const NIX_SSL_CERT_FILE: string;
	export const NIX_STORE: string;
	export const DEVENV_ROOT: string;
	export const LD: string;
	export const NM_FOR_BUILD: string;
	export const NIX_BINTOOLS_FOR_BUILD: string;
	export const pnpm_config_verify_deps_before_run: string;
	export const RUST_SRC_PATH: string;
	export const DIRENV_DIFF: string;
	export const READELF: string;
	export const GETTEXTDATADIRS_FOR_BUILD: string;
	export const STARSHIP_SESSION_KEY: string;
	export const STRIP_FOR_BUILD: string;
	export const NIX_CC_WRAPPER_TARGET_BUILD_x86_64_unknown_linux_gnu: string;
	export const NIX_BINTOOLS_WRAPPER_TARGET_BUILD_x86_64_unknown_linux_gnu: string;
	export const MOZ_GMP_PATH: string;
	export const GHOSTTY_RESOURCES_DIR: string;
	export const XDG_SESSION_CLASS: string;
	export const PYTHONPATH: string;
	export const TERMINFO: string;
	export const TERM: string;
	export const SIZE: string;
	export const OBJCOPY_FOR_BUILD: string;
	export const CC_FOR_BUILD: string;
	export const USER: string;
	export const CARGO_INSTALL_ROOT: string;
	export const AR: string;
	export const AS: string;
	export const NIX_BINTOOLS_WRAPPER_TARGET_HOST_x86_64_unknown_linux_gnu: string;
	export const DISPLAY: string;
	export const OBJDUMP_FOR_BUILD: string;
	export const DEVENV_TASKS: string;
	export const DEVENV_RUNTIME: string;
	export const SHLVL: string;
	export const AR_FOR_BUILD: string;
	export const NM: string;
	export const NIX_LDFLAGS_FOR_BUILD: string;
	export const NIX_CFLAGS_COMPILE: string;
	export const XDG_VTNR: string;
	export const XDG_SESSION_ID: string;
	export const LOCALE_ARCHIVE: string;
	export const npm_config_user_agent: string;
	export const LD_LIBRARY_PATH: string;
	export const PNPM_PACKAGE_NAME: string;
	export const DEVENV_PROFILE: string;
	export const XDG_RUNTIME_DIR: string;
	export const NIX_PKG_CONFIG_WRAPPER_TARGET_HOST_x86_64_unknown_linux_gnu: string;
	export const NODE_PATH: string;
	export const OBJCOPY: string;
	export const RANLIB_FOR_BUILD: string;
	export const DETERMINISTIC_BUILD: string;
	export const DEBUGINFOD_URLS: string;
	export const PGHOST: string;
	export const DEBUGINFOD_IMA_CERT_PATH: string;
	export const PGDATA: string;
	export const BINDGEN_EXTRA_CLANG_ARGS: string;
	export const STRIP: string;
	export const XDG_DATA_DIRS: string;
	export const OBJDUMP: string;
	export const PATH: string;
	export const READELF_FOR_BUILD: string;
	export const CC: string;
	export const GDMSESSION: string;
	export const NIX_CC: string;
	export const DBUS_SESSION_BUS_ADDRESS: string;
	export const STRINGS_FOR_BUILD: string;
	export const DIRENV_WATCHES: string;
	export const NIX_CC_WRAPPER_TARGET_HOST_x86_64_unknown_linux_gnu: string;
	export const DEVENV_STATE: string;
	export const SYSTEMD_SLEEP_FREEZE_USER_SESSIONS: string;
	export const CONFIG_SHELL: string;
	export const RANLIB: string;
	export const NIX_HARDENING_ENABLE: string;
	export const NIX_LDFLAGS: string;
	export const name: string;
	export const NIX_CC_FOR_BUILD: string;
	export const PKG_CONFIG: string;
	export const TERM_PROGRAM: string;
	export const NODE_ENV: string;
}

/**
 * Similar to [`$env/static/private`](https://svelte.dev/docs/kit/$env-static-private), except that it only includes environment variables that begin with [`config.kit.env.publicPrefix`](https://svelte.dev/docs/kit/configuration#env) (which defaults to `PUBLIC_`), and can therefore safely be exposed to client-side code.
 * 
 * Values are replaced statically at build time.
 * 
 * ```ts
 * import { PUBLIC_BASE_URL } from '$env/static/public';
 * ```
 */
declare module '$env/static/public' {
	
}

/**
 * This module provides access to runtime environment variables, as defined by the platform you're running on. For example if you're using [`adapter-node`](https://github.com/sveltejs/kit/tree/main/packages/adapter-node) (or running [`vite preview`](https://svelte.dev/docs/kit/cli)), this is equivalent to `process.env`. This module only includes variables that _do not_ begin with [`config.kit.env.publicPrefix`](https://svelte.dev/docs/kit/configuration#env) _and do_ start with [`config.kit.env.privatePrefix`](https://svelte.dev/docs/kit/configuration#env) (if configured).
 * 
 * This module cannot be imported into client-side code.
 * 
 * ```ts
 * import { env } from '$env/dynamic/private';
 * console.log(env.DEPLOYMENT_SPECIFIC_VARIABLE);
 * ```
 * 
 * > [!NOTE] In `dev`, `$env/dynamic` always includes environment variables from `.env`. In `prod`, this behavior will depend on your adapter.
 */
declare module '$env/dynamic/private' {
	export const env: {
		SHELL: string;
		npm_command: string;
		WINDOWID: string;
		GHOSTTY_BIN_DIR: string;
		COLORTERM: string;
		_PYTHON_HOST_PLATFORM: string;
		hardeningDisable: string;
		TERM_PROGRAM_VERSION: string;
		configureFlags: string;
		DATABASE_URL: string;
		PC_CONFIG_FILES: string;
		mesonFlags: string;
		PKG_CONFIG_PATH: string;
		PYTHONNOUSERSITE: string;
		DEVENV_TASK_FILE: string;
		PGPORT: string;
		PYTHONHASHSEED: string;
		SSH_AUTH_SOCK: string;
		DIRENV_DIR: string;
		npm_config_verify_deps_before_run: string;
		STRINGS: string;
		LD_FOR_BUILD: string;
		NIX_CFLAGS_COMPILE_FOR_BUILD: string;
		DESKTOP_SESSION: string;
		SSH_AGENT_PID: string;
		DIRENV_FILE: string;
		EDITOR: string;
		XDG_SEAT: string;
		PWD: string;
		NIX_PROFILES: string;
		SOURCE_DATE_EPOCH: string;
		LOGNAME: string;
		XDG_SESSION_DESKTOP: string;
		XDG_SESSION_TYPE: string;
		NIX_ENFORCE_NO_NATIVE: string;
		AS_FOR_BUILD: string;
		CXX: string;
		XAUTHORITY: string;
		system: string;
		SIZE_FOR_BUILD: string;
		PC_SOCKET_PATH: string;
		_PYTHON_SYSCONFIGDATA_NAME: string;
		LIBCLANG_PATH: string;
		DEVENV_DOTFILE: string;
		WINDOWPATH: string;
		GDM_LANG: string;
		IN_NIX_SHELL: string;
		GHOSTTY_SHELL_FEATURES: string;
		HOME: string;
		USERNAME: string;
		NIX_BINTOOLS: string;
		LANG: string;
		STARSHIP_SHELL: string;
		cmakeFlags: string;
		CXX_FOR_BUILD: string;
		NIX_SSL_CERT_FILE: string;
		NIX_STORE: string;
		DEVENV_ROOT: string;
		LD: string;
		NM_FOR_BUILD: string;
		NIX_BINTOOLS_FOR_BUILD: string;
		pnpm_config_verify_deps_before_run: string;
		RUST_SRC_PATH: string;
		DIRENV_DIFF: string;
		READELF: string;
		GETTEXTDATADIRS_FOR_BUILD: string;
		STARSHIP_SESSION_KEY: string;
		STRIP_FOR_BUILD: string;
		NIX_CC_WRAPPER_TARGET_BUILD_x86_64_unknown_linux_gnu: string;
		NIX_BINTOOLS_WRAPPER_TARGET_BUILD_x86_64_unknown_linux_gnu: string;
		MOZ_GMP_PATH: string;
		GHOSTTY_RESOURCES_DIR: string;
		XDG_SESSION_CLASS: string;
		PYTHONPATH: string;
		TERMINFO: string;
		TERM: string;
		SIZE: string;
		OBJCOPY_FOR_BUILD: string;
		CC_FOR_BUILD: string;
		USER: string;
		CARGO_INSTALL_ROOT: string;
		AR: string;
		AS: string;
		NIX_BINTOOLS_WRAPPER_TARGET_HOST_x86_64_unknown_linux_gnu: string;
		DISPLAY: string;
		OBJDUMP_FOR_BUILD: string;
		DEVENV_TASKS: string;
		DEVENV_RUNTIME: string;
		SHLVL: string;
		AR_FOR_BUILD: string;
		NM: string;
		NIX_LDFLAGS_FOR_BUILD: string;
		NIX_CFLAGS_COMPILE: string;
		XDG_VTNR: string;
		XDG_SESSION_ID: string;
		LOCALE_ARCHIVE: string;
		npm_config_user_agent: string;
		LD_LIBRARY_PATH: string;
		PNPM_PACKAGE_NAME: string;
		DEVENV_PROFILE: string;
		XDG_RUNTIME_DIR: string;
		NIX_PKG_CONFIG_WRAPPER_TARGET_HOST_x86_64_unknown_linux_gnu: string;
		NODE_PATH: string;
		OBJCOPY: string;
		RANLIB_FOR_BUILD: string;
		DETERMINISTIC_BUILD: string;
		DEBUGINFOD_URLS: string;
		PGHOST: string;
		DEBUGINFOD_IMA_CERT_PATH: string;
		PGDATA: string;
		BINDGEN_EXTRA_CLANG_ARGS: string;
		STRIP: string;
		XDG_DATA_DIRS: string;
		OBJDUMP: string;
		PATH: string;
		READELF_FOR_BUILD: string;
		CC: string;
		GDMSESSION: string;
		NIX_CC: string;
		DBUS_SESSION_BUS_ADDRESS: string;
		STRINGS_FOR_BUILD: string;
		DIRENV_WATCHES: string;
		NIX_CC_WRAPPER_TARGET_HOST_x86_64_unknown_linux_gnu: string;
		DEVENV_STATE: string;
		SYSTEMD_SLEEP_FREEZE_USER_SESSIONS: string;
		CONFIG_SHELL: string;
		RANLIB: string;
		NIX_HARDENING_ENABLE: string;
		NIX_LDFLAGS: string;
		name: string;
		NIX_CC_FOR_BUILD: string;
		PKG_CONFIG: string;
		TERM_PROGRAM: string;
		NODE_ENV: string;
		[key: `PUBLIC_${string}`]: undefined;
		[key: `${string}`]: string | undefined;
	}
}

/**
 * Similar to [`$env/dynamic/private`](https://svelte.dev/docs/kit/$env-dynamic-private), but only includes variables that begin with [`config.kit.env.publicPrefix`](https://svelte.dev/docs/kit/configuration#env) (which defaults to `PUBLIC_`), and can therefore safely be exposed to client-side code.
 * 
 * Note that public dynamic environment variables must all be sent from the server to the client, causing larger network requests — when possible, use `$env/static/public` instead.
 * 
 * ```ts
 * import { env } from '$env/dynamic/public';
 * console.log(env.PUBLIC_DEPLOYMENT_SPECIFIC_VARIABLE);
 * ```
 */
declare module '$env/dynamic/public' {
	export const env: {
		[key: `PUBLIC_${string}`]: string | undefined;
	}
}
