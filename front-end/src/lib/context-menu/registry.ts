import type { ContextPayload, ContextType, MenuItem } from '#lib/components/context-menu/types.ts';
import { lookupProviders } from '#lib/stores/lookupProviders.svelte.ts';
import { transformStore } from '#lib/stores/transforms.svelte.ts';
import { isTerminalStatus } from '#lib/api/format.ts';
import { toasts } from '#lib/stores/toasts.svelte.ts';
import { contextMenuSettings } from '#lib/stores/contextMenuSettings.svelte.ts';
import { valuePreviewStore } from '#lib/stores/valuePreview.svelte.ts';
import { goto, invalidateAll } from '$app/navigation';
import {
	cancelArtifact,
	cancelTask,
	promoteArtifact,
	rescanArtifact,
	rescanSample,
	setArtifactPriority
} from '#lib/api/tasks.ts';
import { isApiError } from '#lib/api/errors.ts';
import type { TaskStatus } from '#lib/api/types.ts';

function defang(value: string): string {
	const { dotReplacement, protocolReplacement } = contextMenuSettings.settings;
	const httpPrefix = protocolReplacement === 'hxxp' ? 'hxxp' : 'hXXp';
	return value
		.replace(/\./g, dotReplacement)
		.replace(/^https:\/\//i, `${httpPrefix}s://`)
		.replace(/^http:\/\//i, `${httpPrefix}://`);
}

function copyAction(label: string, getValue?: (ctx: ContextPayload) => string): MenuItem {
	return {
		kind: 'action',
		id: `copy-${label.toLowerCase().replace(/\s+/g, '-')}`,
		label,
		icon: 'copy',
		handler(ctx) {
			const text = getValue ? getValue(ctx) : ctx.value;
			navigator.clipboard.writeText(text);
		}
	};
}

function maximizeAction(): MenuItem {
	return {
		kind: 'action',
		id: 'maximize',
		label: 'Maximize',
		icon: 'expand',
		handler(ctx) {
			const label = (ctx.metadata?.label as string) ?? 'Value';
			const source = ctx.type === 'table-cell' ? 'Cell value' : 'Value';
			valuePreviewStore.show(ctx.value, label, source);
		}
	};
}

function lookupSubmenu(ctx: ContextPayload): MenuItem | null {
	const subtype = ctx.subtype ?? ctx.type;
	const providers = lookupProviders.getForType(subtype);
	if (providers.length === 0) return null;
	return {
		kind: 'submenu',
		id: 'lookup',
		label: 'Lookup',
		icon: 'lookup',
		children: providers.map((p) => ({
			kind: 'action' as const,
			id: `lookup-${p.id}`,
			label: p.name,
			imageUrl: p.favicon,
			handler() {
				const url = p.urlTemplate.replace('{value}', encodeURIComponent(ctx.value));
				window.open(url, '_blank', 'noopener,noreferrer');
			}
		}))
	};
}

function transformSubmenu(): MenuItem | null {
	const all = transformStore.transforms;
	if (all.length === 0) return null;

	const groups: Record<string, typeof all> = {};
	for (const t of all) {
		(groups[t.category] ??= []).push(t);
	}

	const children: MenuItem[] = [];
	const entries = Object.entries(groups).sort(([a], [b]) => a.localeCompare(b));
	for (const [category, transforms] of entries) {
		children.push({
			kind: 'submenu',
			id: `transform-cat-${category}`,
			label: category.charAt(0).toUpperCase() + category.slice(1),
			children: transforms.map((t) => ({
				kind: 'action' as const,
				id: `transform-${t.id}`,
				label: t.name,
				async handler(ctx: ContextPayload) {
					try {
						const input = new TextEncoder().encode(ctx.value);
						const output = await t.apply(input, {});
						const result = new TextDecoder().decode(output);
						await navigator.clipboard.writeText(result);
						toasts.push({ kind: 'success', message: `${t.name} result copied` });
					} catch (e) {
						toasts.push({
							kind: 'error',
							message: `Transform failed: ${e instanceof Error ? e.message : String(e)}`
						});
					}
				}
			}))
		});
	}

	return {
		kind: 'submenu',
		id: 'transform',
		label: 'Transform',
		icon: 'workbench',
		children
	};
}

function sendToWorkbench(): MenuItem {
	return {
		kind: 'action',
		id: 'send-workbench',
		label: 'Send to Workbench',
		icon: 'workbench',
		handler(ctx) {
			const url = `/workbench?input=${encodeURIComponent(ctx.value)}`;
			window.open(url, '_blank');
		}
	};
}

function customActionItems(contextType: ContextType): MenuItem[] {
	const actions = contextMenuSettings.settings.customActions.filter(
		(a) => a.enabled && a.contextTypes.includes(contextType)
	);
	if (actions.length === 0) return [];
	return actions.map((a) => ({
		kind: 'action' as const,
		id: `custom-${a.id}`,
		label: a.name,
		handler(ctx: ContextPayload) {
			switch (a.actionType) {
				case 'open-url':
					if (a.urlTemplate) {
						const url = a.urlTemplate.replace('{value}', encodeURIComponent(ctx.value));
						window.open(url, '_blank', 'noopener,noreferrer');
					}
					break;
				case 'copy':
					navigator.clipboard.writeText(ctx.value);
					break;
				case 'run-transform':
					if (a.transformId) {
						const t = transformStore.registry.get(a.transformId);
						if (t) {
							t.apply(new TextEncoder().encode(ctx.value), {})
								.then((output) => {
									const result = new TextDecoder().decode(output);
									navigator.clipboard.writeText(result);
									toasts.push({ kind: 'success', message: `${a.name} result copied` });
								})
								.catch((e) => {
									toasts.push({
										kind: 'error',
										message: `${a.name} failed: ${e instanceof Error ? e.message : String(e)}`
									});
								});
						} else {
							toasts.push({ kind: 'error', message: `Transform not found: ${a.transformId}` });
						}
					}
					break;
				case 'run-recipe':
					if (a.recipeId) {
						fetch(`/v1/recipes/${a.recipeId}`)
							.then((r) => r.json())
							.then(
								async (recipe: {
									steps: {
										transform_id: string;
										params: Record<string, string | number | boolean>;
									}[];
								}) => {
									const steps = recipe.steps.map((s) => ({
										transformId: s.transform_id,
										params: s.params,
										source: 'manual' as const
									}));
									const result = await transformStore.pipeline.run(
										new TextEncoder().encode(ctx.value),
										steps
									);
									const text = new TextDecoder().decode(result.output);
									await navigator.clipboard.writeText(text);
									toasts.push({ kind: 'success', message: `${a.name} result copied` });
								}
							)
							.catch((e) => {
								toasts.push({
									kind: 'error',
									message: `${a.name} failed: ${e instanceof Error ? e.message : String(e)}`
								});
							});
					}
					break;
				case 'send-to-workbench': {
					let url = `/workbench?input=${encodeURIComponent(ctx.value)}`;
					if (a.transformId) {
						const steps = JSON.stringify([
							{ transformId: a.transformId, params: {}, source: 'manual' }
						]);
						url += `&steps=${encodeURIComponent(steps)}`;
					}
					window.open(url, '_blank');
					break;
				}
			}
		}
	}));
}

const SEP: MenuItem = { kind: 'separator' };

function errorMessage(e: unknown): string {
	if (isApiError(e)) return e.message;
	return e instanceof Error ? e.message : String(e);
}

/** Runs an API action, reports the outcome as a toast and refreshes page data. */
async function runAction(
	action: () => Promise<unknown>,
	success: string,
	failure: string
): Promise<boolean> {
	try {
		await action();
		toasts.push({ kind: 'success', message: success });
		await invalidateAll();
		return true;
	} catch (e) {
		toasts.push({ kind: 'error', message: `${failure}: ${errorMessage(e)}` });
		return false;
	}
}

function artifactAnalysisItems(ctx: ContextPayload): MenuItem[] {
	const taskId = ctx.metadata?.taskId as number | undefined;
	const artifactId = ctx.metadata?.artifactId as number | undefined;
	if (taskId === undefined || artifactId === undefined) return [];
	const state = ctx.metadata?.state as string | undefined;
	const items: MenuItem[] = [];
	if (state === 'pending' || state === 'running') {
		items.push({
			kind: 'action',
			id: 'artifact-cancel',
			label: 'Cancel analysis',
			icon: 'cancel',
			handler() {
				runAction(
					() => cancelArtifact(fetch, taskId, artifactId),
					`Canceled analysis of ${ctx.value}`,
					'Cancel failed'
				);
			}
		});
	}
	items.push(
		{
			kind: 'action',
			id: 'artifact-rescan',
			label: 'Rescan',
			icon: 'upload',
			handler() {
				runAction(
					() => rescanArtifact(fetch, taskId, artifactId),
					`Rescan of ${ctx.value} queued`,
					'Rescan failed'
				);
			}
		},
		{
			kind: 'action',
			id: 'artifact-promote',
			label: 'Analyse separately',
			icon: 'summary',
			async handler() {
				try {
					const res = await promoteArtifact(fetch, taskId, artifactId);
					toasts.push({ kind: 'success', message: `Task #${res.task_id} submitted` });
					await goto(`/samples/${res.sample.sha256}?run=${res.task_id}`);
				} catch (e) {
					toasts.push({ kind: 'error', message: `Promote failed: ${errorMessage(e)}` });
				}
			}
		},
		{
			kind: 'action',
			id: 'artifact-priority',
			label: 'Raise priority',
			handler() {
				runAction(
					() => setArtifactPriority(fetch, taskId, artifactId, 1000, true),
					`Raised priority of ${ctx.value}`,
					'Priority change failed'
				);
			}
		}
	);
	return items;
}

export function resolveMenuItems(ctx: ContextPayload): MenuItem[] {
	switch (ctx.type) {
		case 'hash':
			return buildHashMenu(ctx);
		case 'indicator':
			return buildIndicatorMenu(ctx);
		case 'table-cell':
			return buildTableCellMenu(ctx);
		case 'artifact':
			return buildArtifactMenu(ctx);
		case 'task':
			return buildTaskMenu(ctx);
		case 'code-block':
			return buildCodeBlockMenu(ctx);
		case 'generic':
		default:
			return buildGenericMenu(ctx);
	}
}

function buildHashMenu(ctx: ContextPayload): MenuItem[] {
	const s = contextMenuSettings.settings;
	const items: MenuItem[] = [copyAction('Copy value')];
	const lookup = lookupSubmenu(ctx);
	if (lookup) {
		items.push(SEP, lookup);
	}
	const custom = customActionItems('hash');
	if (custom.length > 0) items.push(SEP, ...custom);
	if (s.showSendToWorkbench) items.push(SEP, sendToWorkbench());
	return items;
}

function buildIndicatorMenu(ctx: ContextPayload): MenuItem[] {
	const s = contextMenuSettings.settings;
	const items: MenuItem[] = [copyAction('Copy value')];
	if (s.showDefanged && ctx.subtype && s.defangTypes.includes(ctx.subtype)) {
		items.push(copyAction('Copy (defanged)', (c) => defang(c.value)));
	}
	const lookup = lookupSubmenu(ctx);
	if (lookup) {
		items.push(SEP, lookup);
	}
	if (s.showTransformSubmenu) {
		const transform = transformSubmenu();
		if (transform) items.push(transform);
	}
	const custom = customActionItems('indicator');
	if (custom.length > 0) items.push(SEP, ...custom);
	if (s.showSendToWorkbench) items.push(SEP, sendToWorkbench());
	return items;
}

function buildTableCellMenu(ctx: ContextPayload): MenuItem[] {
	const s = contextMenuSettings.settings;
	const items: MenuItem[] = [copyAction('Copy value')];
	if (ctx.metadata?.row) {
		items.push({
			kind: 'action',
			id: 'copy-row',
			label: 'Copy row',
			icon: 'copy',
			handler() {
				const row = ctx.metadata!.row as Record<string, unknown>;
				const text = Object.values(row)
					.map((v) => (v == null ? '' : String(v)))
					.join('\t');
				navigator.clipboard.writeText(text);
			}
		});
	}
	items.push(SEP, maximizeAction());
	if (s.showTransformSubmenu) {
		const transform = transformSubmenu();
		if (transform) items.push(SEP, transform);
	}
	const custom = customActionItems('table-cell');
	if (custom.length > 0) items.push(SEP, ...custom);
	if (s.showSendToWorkbench) items.push(SEP, sendToWorkbench());
	return items;
}

function buildArtifactMenu(ctx: ContextPayload): MenuItem[] {
	const s = contextMenuSettings.settings;
	const items: MenuItem[] = [];
	const previewFn = ctx.metadata?.onPreview as (() => void) | undefined;
	if (previewFn) {
		items.push({
			kind: 'action',
			id: 'preview',
			label: 'Preview',
			icon: 'preview',
			handler: () => previewFn()
		});
	}
	const downloadUrl = ctx.metadata?.downloadUrl as string | undefined;
	if (downloadUrl) {
		items.push({
			kind: 'action',
			id: 'download',
			label: 'Download',
			icon: 'download',
			handler() {
				const a = document.createElement('a');
				a.href = downloadUrl;
				a.download = ctx.value;
				a.click();
			}
		});
	}
	if (items.length > 0) items.push(SEP);
	items.push(copyAction('Copy name'));
	const analysis = artifactAnalysisItems(ctx);
	if (analysis.length > 0) items.push(SEP, ...analysis);
	const custom = customActionItems('artifact');
	if (custom.length > 0) items.push(SEP, ...custom);
	if (s.showSendToWorkbench) items.push(sendToWorkbench());
	return items;
}

function buildTaskMenu(ctx: ContextPayload): MenuItem[] {
	const taskId = ctx.metadata?.taskId as number | undefined;
	const status = ctx.metadata?.status as TaskStatus | undefined;
	const sampleId = ctx.metadata?.sampleId as number | undefined;
	const items: MenuItem[] = [];

	if (taskId) {
		items.push({
			kind: 'action',
			id: 'view-details',
			label: 'View details',
			icon: 'summary',
			handler() {
				window.location.href = `/submissions/${taskId}`;
			}
		});
		items.push(SEP);
		if (sampleId !== undefined) {
			items.push({
				kind: 'action',
				id: 'resubmit',
				label: 'Re-submit',
				icon: 'upload',
				async handler() {
					try {
						const res = await rescanSample(fetch, sampleId);
						toasts.push({ kind: 'success', message: `Task #${res.task_id} submitted` });
						await goto(`/samples/${res.sha256}?run=${res.task_id}`);
					} catch (e) {
						toasts.push({ kind: 'error', message: `Re-submit failed: ${errorMessage(e)}` });
					}
				}
			});
		}
		if (status && !isTerminalStatus(status)) {
			items.push({
				kind: 'action',
				id: 'cancel',
				label: 'Cancel',
				icon: 'cancel',
				handler() {
					runAction(() => cancelTask(fetch, taskId), `Task ${taskId} canceled`, 'Cancel failed');
				}
			});
		}
		items.push(SEP);
		items.push(copyAction('Copy task ID', () => String(taskId)));
	}

	const custom = customActionItems('task');
	if (custom.length > 0) items.push(SEP, ...custom);

	return items;
}

function buildCodeBlockMenu(_ctx: ContextPayload): MenuItem[] {
	const s = contextMenuSettings.settings;
	const items: MenuItem[] = [copyAction('Copy selection')];
	if (s.showTransformSubmenu) {
		const transform = transformSubmenu();
		if (transform) items.push(SEP, transform);
	}
	const custom = customActionItems('code-block');
	if (custom.length > 0) items.push(SEP, ...custom);
	if (s.showSendToWorkbench) items.push(SEP, sendToWorkbench());
	return items;
}

function buildGenericMenu(_ctx: ContextPayload): MenuItem[] {
	const s = contextMenuSettings.settings;
	const items: MenuItem[] = [copyAction('Copy value')];
	items.push(SEP, maximizeAction());
	if (s.showTransformSubmenu) {
		const transform = transformSubmenu();
		if (transform) items.push(SEP, transform);
	}
	const custom = customActionItems('generic');
	if (custom.length > 0) items.push(SEP, ...custom);
	if (s.showSendToWorkbench) items.push(SEP, sendToWorkbench());
	return items;
}
