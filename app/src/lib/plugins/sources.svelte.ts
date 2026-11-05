// The data sources the open event can pull from: those saved on this computer and those saved in
// the event, which win. The tokens aren't in the snapshot, so the app reads these again whenever the
// event changes, and the form updates them.
import { api, type DataSources } from '../api/index';
import type { DataSourceSpec } from './api';

/** The fields a plugin declared optional: they may be left blank and missing from an item. */
export const optionalFields = (spec: DataSourceSpec | undefined) =>
	spec?.fields.filter((f) => f.optional).map((f) => f.key) ?? [];

/**
 * The id a source is saved under: the plugin's, and the tab's when the plugin has one source per
 * tab, like Stock and sales one per cost centre. The details are in the app's data folder by this
 * id, or in the event's plugin record `data-source` (or `data-source:<tab>`); the token is in the
 * keychain by the provider's address and header.
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
