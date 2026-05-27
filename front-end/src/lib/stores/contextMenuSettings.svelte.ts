import type { CustomAction } from '$lib/components/context-menu/types';

const STORAGE_KEY = 'malbox:contextMenuSettings';

type DotReplacement = '[.]' | '(.)' | '{.}';
type ProtocolReplacement = 'hxxp' | 'hXXp';

export type ContextMenuSettingsData = {
	showDefanged: boolean;
	showSendToWorkbench: boolean;
	showTransformSubmenu: boolean;
	dotReplacement: DotReplacement;
	protocolReplacement: ProtocolReplacement;
	defangTypes: string[];
	customActions: CustomAction[];
};

const DEFAULTS: ContextMenuSettingsData = {
	showDefanged: true,
	showSendToWorkbench: true,
	showTransformSubmenu: true,
	dotReplacement: '[.]',
	protocolReplacement: 'hxxp',
	defangTypes: ['ip', 'domain', 'url', 'email'],
	customActions: []
};

function loadFromStorage(): ContextMenuSettingsData {
	if (typeof window === 'undefined') return DEFAULTS;
	const raw = localStorage.getItem(STORAGE_KEY);
	if (!raw) return DEFAULTS;
	try {
		return { ...DEFAULTS, ...JSON.parse(raw) } as ContextMenuSettingsData;
	} catch {
		return DEFAULTS;
	}
}

function saveToStorage(data: ContextMenuSettingsData): void {
	if (typeof window === 'undefined') return;
	localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
}

class ContextMenuSettingsStore {
	private data = $state<ContextMenuSettingsData>(loadFromStorage());

	get settings(): ContextMenuSettingsData {
		return this.data;
	}

	update(changes: Partial<ContextMenuSettingsData>): void {
		this.data = { ...this.data, ...changes };
		saveToStorage(this.data);
	}

	addAction(action: Omit<CustomAction, 'id'>): void {
		const newAction = { ...action, id: crypto.randomUUID() };
		this.data = { ...this.data, customActions: [...this.data.customActions, newAction] };
		saveToStorage(this.data);
	}

	updateAction(id: string, changes: Partial<Omit<CustomAction, 'id'>>): void {
		this.data = {
			...this.data,
			customActions: this.data.customActions.map((a) => (a.id === id ? { ...a, ...changes } : a))
		};
		saveToStorage(this.data);
	}

	removeAction(id: string): void {
		this.data = {
			...this.data,
			customActions: this.data.customActions.filter((a) => a.id !== id)
		};
		saveToStorage(this.data);
	}

	reset(): void {
		this.data = DEFAULTS;
		saveToStorage(this.data);
	}
}

export const contextMenuSettings = new ContextMenuSettingsStore();
