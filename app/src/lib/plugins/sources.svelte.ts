// The data sources saved on this computer. They aren't in the event, so the snapshot doesn't have
// them: the app reads them once and the form updates them.
import { api, type DataSources } from '../api/index';
import type { DataSourceSpec } from './api';

/** The fields a plugin declared optional: they may be left blank and missing from an item. */
export const optionalFields = (spec: DataSourceSpec | undefined) =>
	spec?.fields.filter((f) => f.optional).map((f) => f.key) ?? [];

/**
 * The id a source is saved under: the plugin's, and the tab's when the plugin has one source per
 * tab, like Stock and sales one per cost centre. The details are in the app's data folder and the
 * token is in the keychain, both by this id.
 */
export const sourceId = (plugin: string, tab?: string | null) =>
	tab ? `${plugin}:${tab}` : plugin;

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
