// The data sources saved on this computer. They aren't in the event, so the snapshot doesn't have
// them: the app reads them once and the form updates them.
import { api, type DataSources } from '../api/index';

let current = $state<DataSources>({});

export const dataSources = {
	of: (id: string) => current[id] ?? null,
	/** Without them the app works as ever (R-NF-3), so a failure here isn't one to report. */
	async load() {
		try {
			current = await api.dataSources();
		} catch {
			current = {};
		}
	},
	set(next: DataSources) {
		current = next;
	}
};
