// All a plugin may use of the app (technical design §10). A plugin imports this file, shared
// components and formatting, never the session or the command API, so a sandboxed loader can
// implement the same context later. `boundary.test.ts` enforces it.
import type { Component, Snippet } from 'svelte';
import type { AccountKind } from '../api/generated/AccountKind';
import type { AccountView } from '../api/generated/AccountView';
import type { Assumption } from '../api/generated/Assumption';
import type { Budget } from '../api/generated/Budget';
import type { BudgetLine } from '../api/generated/BudgetLine';
import type { CostCentre } from '../api/generated/CostCentre';
import type { CostCentreFigures } from '../api/generated/CostCentreFigures';
import type { Currency } from '../api/generated/Currency';
import type { NumberFormat } from '../api/generated/NumberFormat';
import type { PluginChange } from '../api/generated/PluginChange';
import type { PluginRecord } from '../api/generated/PluginRecord';
import type { Pulled } from '../api/generated/Pulled';
import type { Table } from '../api/generated/Table';
import type { TableRow } from '../api/generated/TableRow';
import type { TicketBatch } from '../api/generated/TicketBatch';
import type { TierFigures } from '../api/generated/TierFigures';
import type { TxnView } from '../api/generated/TxnView';

export type {
	AccountView,
	Assumption,
	Budget,
	BudgetLine,
	CostCentre,
	CostCentreFigures,
	PluginChange,
	PluginRecord,
	Pulled,
	Table,
	TableRow,
	TicketBatch,
	TierFigures,
	TxnView
};

export type FileDropHandlers = {
	/** A file is being dragged over the window; its name when known, otherwise empty. */
	over: (names: string[]) => void;
	leave: () => void;
	drop: (paths: string[]) => void;
};

/** One line of a proposed expense or income. `amount` is only needed for a split. */
export type ProposedLine = {
	account: number;
	costCentre: number | null;
	amount?: number | null;
};

/**
 * A new entry for the treasurer to check and save in the normal form. What is left out starts as
 * in a new entry: the date as usual, and the event's default account for the payer or destination.
 */
export type EntryProposal = {
	amount: number | null;
	/** ISO `YYYY-MM-DD`. */
	date?: string | null;
	description?: string | null;
	notes?: string | null;
} & (
	| { kind: 'expense'; counterparty: string | null; payer?: number | null; lines: ProposedLine[] }
	| {
			kind: 'income';
			counterparty: string | null;
			destination?: number | null;
			lines: ProposedLine[];
	  }
	| { kind: 'transfer'; from: number | null; to: number | null }
);

export interface PluginContext {
	readonly id: string;
	/** A closed event refuses changes (R-EVT-2). */
	readonly closed: boolean;
	readonly format: NumberFormat;
	/** The event's currency, whose notes and coins a plugin can show. */
	readonly currency: Currency;
	/** What the event holds. People are left out, so plugins don't see bank details. */
	readonly costCentres: CostCentre[];
	readonly accounts: AccountView[];
	readonly txns: TxnView[];
	figuresOf(centre: number): CostCentreFigures | undefined;
	/** Plan, actual and forecast of the whole event, as the Budget page shows them. */
	readonly budget: Budget;
	/** The budget lines, planning data only (R-BUD-5), and the assumptions they scale with. */
	readonly budgetLines: BudgetLine[];
	readonly assumptions: Assumption[];
	/** The ticket tiers with what was sold of each. No orders, so no one's purchase. */
	readonly tiers: TierFigures[];
	/**
	 * Whether the event's ticket imports have order IDs; null until one has tickets. A batch of the
	 * other kind is refused. The orders themselves stay out of plugins.
	 */
	readonly ticketOrderIds: boolean | null;
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
	/**
	 * Reads only these columns of an `.ods`, `.xlsx` or `.csv`, and hashes the file. Everything else
	 * in it stays out of the plugin. Rejects when a column isn't there.
	 */
	readTable(path: string, columns: string[]): Promise<Table>;
	/** A cell or typed text as cents, by the app's number format. Rejects what it can't read. */
	parseAmount(text: string): Promise<number>;
	/** A cell or typed text as an ISO date. Rejects what it can't read. */
	parseDate(text: string): Promise<string>;
	/**
	 * What this plugin's data source is called, or the one of a tab (with the tab's key); null
	 * until it is set up in Settings. Without `sourceTabs` there is only the one.
	 */
	dataSource(tab?: string): string | null;
	/**
	 * Asks the data source, or the one of a tab, for these fields. Nothing else in its answer
	 * reaches the plugin. A field declared optional is left out of a row that lacks it, and out of
	 * `fields` when the source has no name for it.
	 */
	pull(fields: string[], tab?: string): Promise<Pulled>;
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

/** On the Budget page while the event is open. */
export type BudgetSectionProps = { ctx: PluginContext };

/** Under one of the amounts in the panel that counts an account. */
export type CountSectionProps = {
	ctx: PluginContext;
	account: number;
	which: 'opening' | 'closing';
	/** What the amount field holds now. */
	amount: number | null;
	/** Fills in the amount field; nothing is saved until the treasurer does. */
	setAmount: (amount: number | null) => void;
	/** Runs this once the count is saved, so Cancel leaves nothing. Returns a function that stops. */
	onSaved: (run: () => Promise<void>) => () => void;
};

/** Printed in the settlement report: it only shows, it changes nothing. */
export type ReportSectionProps = { ctx: PluginContext };

/** What a plugin wants from a provider. The app draws the form for it in Settings. */
export type DataSourceSpec = {
	/** The usual name of the provider's list. */
	list: string;
	/** An optional field may be left blank in Settings, and missing from an item. */
	fields: { key: string; label: string; usually: string; optional?: boolean }[];
};

/**
 * A tab of the Configure panel with a data source of its own, like a cost centre for Stock and
 * sales: the drinks and the merch can come from different providers.
 */
export type SourceTab = {
	/**
	 * What the source is saved under, apart from the plugin's other tabs. A cost centre's name is
	 * one, so an event with a cost centre of that name pulls from the same source.
	 */
	key: string;
	/** What the panel calls it. */
	title: string;
};

export interface Plugin {
	/** Stored with its records: never changes. Lower-case letters, digits and hyphens. */
	id: string;
	name: string;
	/** One sentence for Settings: what it adds to the app. */
	description: string;
	/**
	 * One line for its card in Settings: how it is set up, such as the source it pulls from. Left
	 * out, or null, when it has nothing to set up.
	 */
	summary?(ctx: PluginContext): string | null;
	/** Whether `CentreSection` shows on this cost centre's page. */
	attachedTo?(ctx: PluginContext, centre: number): boolean;
	CentreSection?: Component<CentreSectionProps>;
	/** On the Tickets page: a way for tickets to come in. */
	TicketSection?: Component<TicketSectionProps>;
	SettingsSection?: Component<SettingsSectionProps>;
	/** Whether `BudgetSection` has something to show. Yes when left out. */
	onBudget?(ctx: PluginContext): boolean;
	BudgetSection?: Component<BudgetSectionProps>;
	/** Whether `CountSection` shows for this account. Yes when left out. */
	onCount?(ctx: PluginContext, account: number): boolean;
	CountSection?: Component<CountSectionProps>;
	/** Whether `ReportSection` has something to print. Yes when left out. */
	inReport?(ctx: PluginContext): boolean;
	ReportSection?: Component<ReportSectionProps>;
	/** Offers a data source. `TicketSection` then waits until it is set up. */
	dataSource?: DataSourceSpec;
	/**
	 * One data source per entry, such as one per cost centre it is shown on: each is set up in its
	 * own tab of the Configure panel, and `dataSource(key)` and `pull(fields, key)` ask for one.
	 * Left out, the plugin has the one source it declared.
	 */
	sourceTabs?(ctx: PluginContext): SourceTab[];
}
