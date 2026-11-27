import { describe, it, expect } from 'vitest';
import { buildArtifactTree } from './tree';
import type {
	ArtifactEdge,
	ArtifactRelation,
	TaskArtifact,
	TaskJob,
	TaskTree
} from '#lib/api/types.ts';

function artifact(id: number, name: string, depth = 0): TaskArtifact {
	return {
		id,
		sha256: String(id).padStart(64, '0'),
		size_bytes: 10,
		analyze: true,
		sample_id: id,
		depth,
		artifact_type: 'text/plain',
		mime: 'text/plain',
		state: 'done',
		drop_reason: null,
		priority: 1,
		canceled_by: null,
		promoted_task_id: null,
		name,
		relation: depth === 0 ? 'root' : 'extracted',
		verdict: null,
		content_url: `/v1/tasks/1/artifacts/${id}/content`
	};
}

function edge(
	parent: number,
	child: number,
	relation: ArtifactRelation = 'extracted'
): ArtifactEdge {
	return {
		parent_artifact_id: parent,
		child_artifact_id: child,
		relation,
		name: null,
		source_job_id: null,
		source_plugin: null
	};
}

function job(id: number, artifactId: number, analyzer: string): TaskJob {
	return {
		id,
		artifact_id: artifactId,
		kind: 'host',
		analyzer,
		stage: 'static',
		state: 'completed',
		attempts: 1,
		max_attempts: 3,
		reruns: 0,
		error_class: null,
		error_message: null,
		cached_from_job_id: null,
		machine_id: null,
		started_on: null,
		completed_on: null
	};
}

function tree(partial: Partial<TaskTree>): TaskTree {
	return { task_id: 1, root_artifact_id: 1, artifacts: [], edges: [], jobs: [], ...partial };
}

describe('buildArtifactTree', () => {
	it('builds a single root', () => {
		const t = buildArtifactTree(tree({ artifacts: [artifact(1, 'a.zip')] }));
		expect(t).not.toBeNull();
		expect(t!.relation).toBe('root');
		expect(t!.name).toBe('a.zip');
		expect(t!.children).toEqual([]);
		expect(t!.alsoFrom).toEqual([]);
	});

	it('builds a two-level tree with siblings ordered by id', () => {
		const t = buildArtifactTree(
			tree({
				artifacts: [
					artifact(1, 'a.zip'),
					artifact(3, 'y.txt', 1),
					artifact(2, 'x.exe', 1),
					artifact(4, 'inner.bin', 2)
				],
				edges: [edge(1, 3), edge(1, 2, 'dropped'), edge(2, 4, 'unpacked')]
			})
		)!;
		expect(t.children.map((c) => c.artifact.id)).toEqual([2, 3]);
		expect(t.children[0].relation).toBe('dropped');
		expect(t.children[0].children.map((c) => c.name)).toEqual(['inner.bin']);
		expect(t.children[0].children[0].relation).toBe('unpacked');
		expect(t.children[1].children).toEqual([]);
	});

	it('shows a DAG child once under its earliest parent and fills alsoFrom', () => {
		const t = buildArtifactTree(
			tree({
				artifacts: [
					artifact(1, 'root.zip'),
					artifact(2, 'a.zip', 1),
					artifact(3, 'b.zip', 1),
					artifact(4, 'shared.exe', 2)
				],
				edges: [edge(1, 2), edge(1, 3), edge(3, 4), edge(2, 4), edge(1, 1)]
			})
		)!;
		const [a, b] = t.children;
		expect(a.children).toEqual([]);
		expect(b.children.map((c) => c.artifact.id)).toEqual([4]);
		expect(b.children[0].alsoFrom).toEqual([2]);
		expect(t.alsoFrom).toEqual([]);
	});

	it('groups jobs per artifact', () => {
		const t = buildArtifactTree(
			tree({
				artifacts: [artifact(1, 'a.zip'), artifact(2, 'x.exe', 1)],
				edges: [edge(1, 2)],
				jobs: [job(10, 1, 'extractor'), job(11, 2, 'pe'), job(12, 2, 'strings')]
			})
		)!;
		expect(t.jobs.map((j) => j.analyzer)).toEqual(['extractor']);
		expect(t.children[0].jobs.map((j) => j.analyzer)).toEqual(['pe', 'strings']);
	});

	it('returns null when the root is missing', () => {
		expect(buildArtifactTree(tree({ root_artifact_id: null }))).toBeNull();
		expect(
			buildArtifactTree(tree({ root_artifact_id: 9, artifacts: [artifact(1, 'a')] }))
		).toBeNull();
	});
});
