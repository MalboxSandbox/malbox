import type { ArtifactRelation, TaskArtifact, TaskJob, TaskTree } from '#lib/api/types.ts';

export interface ArtifactTreeNode {
	artifact: TaskArtifact;
	relation: ArtifactRelation | 'root';
	name: string;
	jobs: TaskJob[];
	children: ArtifactTreeNode[];
	/** Ids of additional parents (the node is shown once, under its earliest parent). */
	alsoFrom: number[];
}

/**
 * Turn the artifact DAG returned by `GET /v1/tasks/{id}/tree` into a tree.
 *
 * Each artifact appears once, under its earliest parent edge (edge order in
 * `tree.edges`, which the API returns by creation time). Other parents are
 * listed in `alsoFrom`. Siblings are ordered by artifact id.
 */
export function buildArtifactTree(tree: TaskTree): ArtifactTreeNode | null {
	if (tree.root_artifact_id === null) return null;
	const rootId = tree.root_artifact_id;

	const artifacts = new Map<number, TaskArtifact>();
	for (const a of tree.artifacts) artifacts.set(a.id, a);
	const root = artifacts.get(rootId);
	if (!root) return null;

	const jobsByArtifact = new Map<number, TaskJob[]>();
	for (const j of tree.jobs) {
		const list = jobsByArtifact.get(j.artifact_id);
		if (list) list.push(j);
		else jobsByArtifact.set(j.artifact_id, [j]);
	}

	const primary = new Map<number, { parent: number; relation: ArtifactRelation }>();
	const alsoFrom = new Map<number, number[]>();
	const childrenOf = new Map<number, number[]>();

	for (const e of tree.edges) {
		const child = e.child_artifact_id;
		const parent = e.parent_artifact_id;
		if (child === parent || !artifacts.has(child) || !artifacts.has(parent)) continue;
		const existing = primary.get(child);
		if (child !== rootId && !existing) {
			primary.set(child, { parent, relation: e.relation });
			const siblings = childrenOf.get(parent);
			if (siblings) siblings.push(child);
			else childrenOf.set(parent, [child]);
			continue;
		}
		if (existing?.parent === parent) continue;
		const extra = alsoFrom.get(child) ?? [];
		if (!extra.includes(parent)) extra.push(parent);
		alsoFrom.set(child, extra);
	}

	const visited = new Set<number>();

	function build(id: number, relation: ArtifactRelation | 'root'): ArtifactTreeNode {
		visited.add(id);
		const artifact = artifacts.get(id)!;
		const childIds = [...(childrenOf.get(id) ?? [])].sort((a, b) => a - b);
		const children: ArtifactTreeNode[] = [];
		for (const c of childIds) {
			if (visited.has(c)) continue;
			children.push(build(c, primary.get(c)!.relation));
		}
		return {
			artifact,
			relation,
			name: artifact.name,
			jobs: jobsByArtifact.get(id) ?? [],
			children,
			alsoFrom: alsoFrom.get(id) ?? []
		};
	}

	return build(rootId, 'root');
}
