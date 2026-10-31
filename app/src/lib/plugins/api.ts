// All a plugin may use of the app (technical design §10). A plugin imports this file, shared
// components and formatting, never the session or the command API, so a sandboxed loader can
// implement the same context later. `boundary.test.ts` enforces it.
import type { Component, Snippet } from 'svelte';
import type { AccountKind } from '../api/generated/AccountKind';
import type { AccountView } from '../api/generated/AccountView';
import type { CostCentre } from '../api/generated/CostCentre';
import type { CostCentreFigures } from '../api/generated/CostCentreFigures';
import type { NumberFormat } from '../api/generated/NumberFormat';
import type { PluginChange } from '../api/generated/PluginChange';
import type { PluginRecord } from '../api/generated/PluginRecord';
import type { Sheet } from '../api/generated/Sheet';
import type { TicketBatch } from '../api/generated/TicketBatch';
import type { TicketColumns } from '../api/generated/TicketColumns';
import type { TxnView } from '../api/generated/TxnView';

export type {
	AccountView,
	CostCentre,
	CostCentreFigures,
	PluginChange,
	PluginRecord,
	Sheet,
	TicketBatch,
	TicketColumns,
	TxnView
};

export type FileDropHandlers = {
	/** A file is being dragged over the window; its name when known, otherwise empty. */
	over: (names: string[]) => void;
	leave: () => void;
	drop: (paths: string[]) => void;
};

/** A new expense for the treasurer to check and save in the normal form. */
export type EntryProposal = {
	counterparty: string | null;
	amount: number | null;
	lines: { account: number; costCentre: number | null }[];
};

export interface PluginContext {
	readonly id: string;
	/** A closed event refuses changes (R-EVT-2). */
	readonly closed: boolean;
	readonly format: NumberFormat;
	/** What the event holds. People are left out, so plugins don't see bank details. */
	readonly costCentres: CostCentre[];
	readonly accounts: AccountView[];
	readonly txns: TxnView[];
	figuresOf(centre: number): CostCentreFigures | undefined;
	/** This plugin's records, in creation order. */
	readonly records: PluginRecord[];
	/** Applies the changes together. A null value removes a record. */
	apply(changes: PluginChange[]): Promise<void>;
	createCostCentre(name: string): Promise<number>;
	/** A new category whose lines start in the cost centre. */
	createCategory(kind: AccountKind, name: string, costCentre: number): Promise<number>;
	/** What new lines of an existing category start with. Existing lines keep theirs. */
	setCategoryCentre(category: number, costCentre: number): Promise<void>;
	/** Opens the normal entry form with this filled in; nothing is saved until the treasurer does. */
	proposeEntry(proposal: EntryProposal): void;
	/** Asks the treasurer for a file; null when cancelled. */
	chooseFile(name: string, extensions: string[]): Promise<string | null>;
	/** Listens for files dragged onto the window; returns a function that stops. */
	onFileDrop(handlers: FileDropHandlers): () => void;
	/** Reads only these columns of a spreadsheet, and hashes the file. */
	readTicketSheet(path: string, columns: TicketColumns): Promise<Sheet>;
}

export type CentreSectionProps = {
	ctx: PluginContext;
	centre: CostCentre;
	/** Shows a right-hand panel beside the page, or removes it. */
	setPanel: (panel: Snippet | null) => void;
};

export type TicketSectionProps = {
	ctx: PluginContext;
	/** Opens the import's review with these tickets; nothing is saved until the treasurer does. */
	proposeImport: (batch: TicketBatch) => void;
	/** Shows a right-hand panel beside the page, or removes it. */
	setPanel: (panel: Snippet | null) => void;
};

export type SettingsSectionProps = { ctx: PluginContext };

export interface Plugin {
	/** Stored with its records: never changes. Lower-case letters, digits and hyphens. */
	id: string;
	name: string;
	/** One sentence for Settings: what it adds to the app. */
	description: string;
	/** Whether `CentreSection` shows on this cost centre's page. */
	attachedTo?(ctx: PluginContext, centre: number): boolean;
	CentreSection?: Component<CentreSectionProps>;
	/** On the Tickets page: a way for tickets to come in. */
	TicketSection?: Component<TicketSectionProps>;
	SettingsSection?: Component<SettingsSectionProps>;
}
