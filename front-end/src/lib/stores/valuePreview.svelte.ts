class ValuePreviewStore {
	open = $state(false);
	value = $state('');
	label = $state('');
	source = $state('');

	show(value: string, label: string, source: string): void {
		this.value = value;
		this.label = label;
		this.source = source;
		this.open = true;
	}

	close(): void {
		this.open = false;
		this.value = '';
		this.label = '';
		this.source = '';
	}
}

export const valuePreviewStore = new ValuePreviewStore();
