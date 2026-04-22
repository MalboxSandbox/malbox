import { browser } from '$app/environment';

export interface PollingOptions {
	intervalMs: number;
	pauseWhenHidden?: boolean;
	onError?: (err: unknown) => void;
}

/**
 * Starts polling `fn` on an interval. Returns a stop function.
 * `fn` may be async; its errors are swallowed (optionally reported via onError)
 * so polling never throws out of the effect.
 *
 * When `pauseWhenHidden` is true, polling pauses while `document.hidden` is true
 * and resumes on visibilitychange.
 */
export function startPolling(fn: () => void | Promise<void>, opts: PollingOptions): () => void {
	if (!browser) return () => {};

	let stopped = false;
	let timer: ReturnType<typeof setTimeout> | null = null;

	async function tick() {
		if (stopped) return;
		if (opts.pauseWhenHidden && document.hidden) {
			schedule();
			return;
		}
		try {
			await fn();
		} catch (err) {
			opts.onError?.(err);
		}
		schedule();
	}

	function schedule() {
		if (stopped) return;
		timer = setTimeout(tick, opts.intervalMs);
	}

	function onVisibilityChange() {
		if (!document.hidden && !stopped && timer === null) {
			schedule();
		}
	}

	if (opts.pauseWhenHidden) {
		document.addEventListener('visibilitychange', onVisibilityChange);
	}

	schedule();

	return () => {
		stopped = true;
		if (timer) clearTimeout(timer);
		timer = null;
		if (opts.pauseWhenHidden) {
			document.removeEventListener('visibilitychange', onVisibilityChange);
		}
	};
}
