import { requestJson, type FetchLike } from './client';
import type { SampleOverviewResponse, SampleReportResponse } from './types';

export async function getSampleOverview(
	fetchFn: FetchLike,
	sha256: string
): Promise<SampleOverviewResponse> {
	return requestJson<SampleOverviewResponse>(fetchFn, `/v1/samples/${encodeURIComponent(sha256)}`);
}

export async function getSampleReport(
	fetchFn: FetchLike,
	sha256: string
): Promise<SampleReportResponse> {
	return requestJson<SampleReportResponse>(
		fetchFn,
		`/v1/samples/${encodeURIComponent(sha256)}/report`
	);
}
