// The app's side of the plugin boundary: `PluginContext`, built on the session and the commands.
import { goto } from '$app/navigation';
import { api } from '../api/index';
import { session } from '../state/session.svelte';
import type { EntryProposal, PluginContext } from './api';

/** The app's own record in a plugin's namespace: present when the plugin is switched off. */
const OFF_KEY = 'host:off';

let proposal: EntryProposal | null = null;

/** The proposal waiting for the ledger to open it. It is handed out once. */
export function takeProposal(): EntryProposal | null {
	const taken = proposal;
	proposal = null;
	return taken;
}

/** On unless switched off for this event. Off hides a plugin; its records stay in the file. */
export function pluginEnabled(id: string): boolean {
	return !session.event!.snapshot.pluginRecords.some((r) => r.plugin === id && r.key === OFF_KEY);
}

export async function setPluginEnabled(id: string, on: boolean) {
	await session.change(() =>
		api.applyPluginRecords(id, [{ key: OFF_KEY, scope: 'setup', value: on ? null : true }])
	);
}

export function pluginContext(id: string): PluginContext {
	const snapshot = () => session.event!.snapshot;
	return {
		id,
		get closed() {
			return session.closed;
		},
		get format() {
			return session.format;
		},
		get costCentres() {
			return snapshot().costCentres;
		},
		get accounts() {
			return snapshot().accounts;
		},
		get txns() {
			return snapshot().txns;
		},
		figuresOf: (centre) => snapshot().budget.costCentres.find((c) => c.costCentre === centre),
		get records() {
			return snapshot().pluginRecords.filter((r) => r.plugin === id && r.key !== OFF_KEY);
		},
		async apply(changes) {
			await session.change(() => api.applyPluginRecords(id, changes));
		},
		async createCostCentre(name) {
			const known = new Set(snapshot().costCentres.map((c) => c.id));
			const event = await session.change(() => api.addCostCentre(name));
			return event.snapshot.costCentres.find((c) => !known.has(c.id))!.id;
		},
		async createCategory(kind, name, costCentre) {
			const added = await session.change(() =>
				api.addAccount(kind === 'income' ? 'income' : 'expense', name)
			);
			await session.change(() => api.setCategoryCostCentre(added.id, costCentre));
			return added.id;
		},
		async setCategoryCentre(category, costCentre) {
			await session.change(() => api.setCategoryCostCentre(category, costCentre));
		},
		proposeEntry(next) {
			proposal = next;
			goto('/event/ledger?proposal');
		}
	};
}
