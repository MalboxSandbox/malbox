export type ContextType =
	| 'hash'
	| 'indicator'
	| 'table-cell'
	| 'artifact'
	| 'task'
	| 'code-block'
	| 'generic';

export type ContextPayload = {
	type: ContextType;
	value: string;
	subtype?: string;
	metadata?: Record<string, unknown>;
};

export type MenuItemAction = {
	kind: 'action';
	id: string;
	label: string;
	icon?: string;
	imageUrl?: string;
	shortcut?: string;
	disabled?: boolean;
	handler: (context: ContextPayload) => void;
};

export type MenuItemSubmenu = {
	kind: 'submenu';
	id: string;
	label: string;
	icon?: string;
	children: MenuItem[];
};

export type MenuItemSeparator = {
	kind: 'separator';
};

export type MenuItem = MenuItemAction | MenuItemSubmenu | MenuItemSeparator;

export type LookupProvider = {
	id: string;
	name: string;
	urlTemplate: string;
	indicatorTypes: string[];
	icon?: string;
	favicon?: string;
	enabled: boolean;
};

export type CustomActionType =
	| 'open-url'
	| 'copy'
	| 'run-transform'
	| 'run-recipe'
	| 'send-to-workbench';

export type CustomAction = {
	id: string;
	name: string;
	actionType: CustomActionType;
	urlTemplate?: string;
	transformId?: string;
	recipeId?: string;
	contextTypes: ContextType[];
	enabled: boolean;
};
