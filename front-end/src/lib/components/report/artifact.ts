import type { ArtifactLink } from '$lib/api/types';

/**
 * Resolve an artifact name (as emitted by a plugin inside `image`/`download`
 * blocks or `ArtifactRef`) to its download URL in the current task context.
 * Returns null when the plugin references an artifact that wasn't produced.
 */
export function resolveArtifactUrl(
	name: string,
	artifacts: ArtifactLink[]
): string | null {
	return artifacts.find((a) => a.result_name === name)?.url ?? null;
}
