import { request, requestJson, type FetchLike } from './client';
import type { Machine, Snapshot, ProvisionRun, ProvisionMachineRequest } from './types';

export async function listMachines(fetchFn: FetchLike): Promise<Machine[]> {
	return requestJson<Machine[]>(fetchFn, '/api/machines');
}

export async function getMachine(fetchFn: FetchLike, id: number): Promise<Machine> {
	return requestJson<Machine>(fetchFn, `/api/machines/${id}`);
}

export async function listSnapshots(fetchFn: FetchLike, machineId: number): Promise<Snapshot[]> {
	return requestJson<Snapshot[]>(fetchFn, `/api/machines/${machineId}/snapshots`);
}

export async function deleteSnapshot(
	fetchFn: FetchLike,
	machineId: number,
	name: string
): Promise<void> {
	await request(fetchFn, `/api/machines/${machineId}/snapshots/${encodeURIComponent(name)}`, {
		method: 'DELETE'
	});
}

export async function provisionMachine(
	fetchFn: FetchLike,
	machineId: number,
	req: ProvisionMachineRequest
): Promise<ProvisionRun> {
	return requestJson<ProvisionRun>(fetchFn, `/api/machines/${machineId}/provision`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify(req)
	});
}

export async function listProvisions(
	fetchFn: FetchLike,
	machineId: number
): Promise<ProvisionRun[]> {
	return requestJson<ProvisionRun[]>(fetchFn, `/api/machines/${machineId}/provisions`);
}
