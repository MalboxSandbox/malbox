let collapsed = $state(false);

export const sidebar = {
	get collapsed() {
		return collapsed;
	},
	toggle() {
		collapsed = !collapsed;
	}
};
