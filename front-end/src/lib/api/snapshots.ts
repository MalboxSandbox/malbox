import { requestJson, type FetchLike } from './client';
import type { SnapshotForSubmission, Platform } from './types';

export async function listSnapshotsForSubmission(
	fetchFn: FetchLike,
	platform: Platform
): Promise<SnapshotForSubmission[]> {
	return requestJson<SnapshotForSubmission[]>(
		fetchFn,
		`/v1/snapshots?platform=${encodeURIComponent(platform)}`
	);
}
