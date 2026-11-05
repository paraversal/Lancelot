// Typed wrappers around the Tauri commands (docs/technical-design.md §5).
// The types are generated from Rust by ts-rs; see ./generated.
import { convertFileSrc, invoke as tauriInvoke, isTauri } from '@tauri-apps/api/core';

import type { AccountId } from './generated/AccountId';
import type { AccountDraft } from './generated/AccountDraft';
import type { AccountKind } from './generated/AccountKind';
import type { Attachment } from './generated/Attachment';
import type { AssetType } from './generated/AssetType';
import type { AssumptionId } from './generated/AssumptionId';
import type { AttachmentId } from './generated/AttachmentId';
import type { Added } from './generated/Added';
import type { ApiError as ApiErrorData } from './generated/ApiError';
import type { AppInfo } from './generated/AppInfo';
import type { AppSettings } from './generated/AppSettings';
import type { BackupInfo } from './generated/BackupInfo';
import type { BlockerOverride } from './generated/BlockerOverride';
import type { BudgetLineDraft } from './generated/BudgetLineDraft';
import type { BudgetLineId } from './generated/BudgetLineId';
import type { CostCentreId } from './generated/CostCentreId';
import type { Cents } from './generated/Cents';
import type { CopyChoice } from './generated/CopyChoice';
import type { CopySummary } from './generated/CopySummary';
import type { TemplateSummary } from './generated/TemplateSummary';
import type { ReconciliationId } from './generated/ReconciliationId';
import type { CancellationDraft } from './generated/CancellationDraft';
import type { CancellationEdit } from './generated/CancellationEdit';
import type { CancellationId } from './generated/CancellationId';
import type { ChangeLogEntry } from './generated/ChangeLogEntry';
import type { CheckedIban } from './generated/CheckedIban';
import type { CountInput } from './generated/CountInput';
import type { EventDetails } from './generated/EventDetails';
import type { ImportDeletion } from './generated/ImportDeletion';
import type { ImportId } from './generated/ImportId';
import type { ImportPreview } from './generated/ImportPreview';
import type { ManualSaleDraft } from './generated/ManualSaleDraft';
import type { ManualSaleId } from './generated/ManualSaleId';
import type { OpenEvent } from './generated/OpenEvent';
import type { PluginChange } from './generated/PluginChange';
import type { Parsed } from './generated/Parsed';
import type { ParsedSum } from './generated/ParsedSum';
import type { Person } from './generated/Person';
import type { ParsedDate } from './generated/ParsedDate';
import type { RecentEvent } from './generated/RecentEvent';
import type { Report } from './generated/Report';
import type { Resolution } from './generated/Resolution';
import type { SavedTxn } from './generated/SavedTxn';
import type { DataSource } from './generated/DataSource';
import type { DataSourceView } from './generated/DataSourceView';
import type { Pulled } from './generated/Pulled';
import type { Table } from './generated/Table';
import type { TicketBatch } from './generated/TicketBatch';
import type { TicketShare } from './generated/TicketShare';
import type { TierDraft } from './generated/TierDraft';
import type { TierId } from './generated/TierId';
import type { TxnDraft } from './generated/TxnDraft';
import type { TxnId } from './generated/TxnId';

export type DataSources = { [id: string]: DataSourceView | undefined };

/** A failed command. `code` is stable; `message` is for people. */
export class ApiError extends Error {
	readonly code: string;
	readonly field: string | null;
	readonly report: Report | null;

	constructor(data: ApiErrorData) {
		super(data.message);
		this.name = 'ApiError';
		this.code = data.code;
		this.field = data.field;
		this.report = data.report;
	}
}

/** What to show for a failure of any kind. */
export function errorMessage(e: unknown): string {
	return e instanceof ApiError ? e.message : String(e);
}

function isErrorData(value: unknown): value is ApiErrorData {
	return typeof value === 'object' && value !== null && 'code' in value && 'message' in value;
}

type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

/** Tauri's invoke in the app; in a plain browser during development, the dev bridge. */
async function backend(): Promise<Invoke> {
	if (isTauri()) return tauriInvoke;
	if (import.meta.env.DEV) return (await import('./dev')).invoke;
	throw new ApiError({
		code: 'internal',
		message: 'not running in Lancelot',
		field: null,
		report: null
	});
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
	const invoke = await backend();
	try {
		return await invoke<T>(command, args);
	} catch (e) {
		if (e instanceof ApiError) throw e;
		if (isErrorData(e)) throw new ApiError(e);
		throw new ApiError({ code: 'internal', message: String(e), field: null, report: null });
	}
}

export const api = {
	appInfo: () => call<AppInfo>('app_info'),

	settings: () => call<AppSettings>('settings_get'),
	setSettings: (settings: AppSettings) => call<AppSettings>('settings_set', { settings }),
	recentEvents: () => call<RecentEvent[]>('recent_events'),
	forgetRecent: (path: string) => call<RecentEvent[]>('recent_forget', { path }),

	/** With a template, the new event starts with that event's setup instead (R-EVT-3). */
	createEvent: (
		path: string,
		details: EventDetails,
		accounts: AccountDraft[],
		template: string | null = null
	) => call<OpenEvent>('event_create', { path, details, accounts, template }),
	templatePreview: (path: string) => call<TemplateSummary>('template_preview', { path }),
	/** What copying everything from that event would add; changes nothing. */
	copyPreview: (path: string) => call<CopySummary>('copy_preview', { path }),
	copyFromEvent: (path: string, choice: CopyChoice) =>
		call<OpenEvent>('copy_from_event', { path, choice }),
	openEvent: (path: string) => call<OpenEvent>('event_open', { path }),
	/** Closes the file and releases its lock; closing the event itself is `closeEvent`. */
	closeFile: () => call<void>('file_close'),
	currentEvent: () => call<OpenEvent | null>('event_current'),
	updateEvent: (details: EventDetails) => call<OpenEvent>('event_update', { details }),
	setDefaultReimbursementAccount: (account: AccountId) =>
		call<OpenEvent>('event_set_default_reimbursement_account', { account }),

	checkIntegrity: () => call<OpenEvent>('integrity_check'),
	/** The open event's change log, newest first (R-TX-6). */
	changeLog: () => call<ChangeLogEntry[]>('change_log_list'),
	/** Backups of an event file: the open one, or one that cannot be opened. */
	backups: (event: string) => call<BackupInfo[]>('backups_list', { event }),
	restoreBackup: (event: string, backup: string, target: string) =>
		call<OpenEvent>('backup_restore', { event, backup, target }),

	/** A percentage of `count` tickets by the core's rule; changes nothing. */
	ticketShare: (price: Cents, bp: number, count: number) =>
		call<TicketShare>('ticket_share', { price, bp, count }),
	parseAmount: (input: string, allowNegative: boolean) =>
		call<Parsed>('amount_parse', { input, allowNegative }),
	/** What an amount field holds, which may be a sum. File cells go through `parseAmount`. */
	parseSum: (input: string, allowNegative: boolean) =>
		call<ParsedSum>('amount_sum', { input, allowNegative }),
	parseDate: (input: string) => call<ParsedDate>('date_parse', { input }),

	/** Saves a transaction with the files to attach (paths) and the attachments to remove. */
	saveTxn: (draft: TxnDraft, id: TxnId | null, files: string[] = [], remove: AttachmentId[] = []) =>
		call<SavedTxn>('txn_save', { draft, id, files, remove }),
	openAttachment: (id: AttachmentId) => call<void>('attachment_open', { id }),
	exportAttachment: (id: AttachmentId, target: string) =>
		call<void>('attachment_export', { id, target }),
	deleteTxn: (id: TxnId) => call<OpenEvent>('txn_delete', { id }),
	addAccount: (kind: AccountKind, name: string) => call<Added>('account_add', { kind, name }),
	renameAccount: (id: AccountId, name: string) => call<OpenEvent>('account_rename', { id, name }),
	/** Only an account or person nothing refers to. */
	deleteAccount: (id: AccountId) => call<OpenEvent>('account_delete', { id }),
	addAsset: (name: string, assetType: AssetType) => call<Added>('asset_add', { name, assetType }),
	setAssetType: (id: AccountId, assetType: AssetType) =>
		call<OpenEvent>('asset_set_type', { id, assetType }),
	/** Its counts and statements leave out opening counts (R-ISO-2). */
	setNetMovement: (id: AccountId, netMovement: boolean) =>
		call<OpenEvent>('asset_set_net_movement', { id, netMovement }),
	/** Replaces both counts of an asset account; `null` removes one. */
	setCounts: (account: AccountId, opening: CountInput | null, closing: CountInput | null) =>
		call<OpenEvent>('counts_set', { account, opening, closing }),
	/** What a statement shows on a date (R-ACC-4). On a net-movement account: the net movement. */
	addReconciliation: (account: AccountId, date: string, amount: Cents) =>
		call<OpenEvent>('reconciliation_add', { account, date, amount }),
	updateReconciliation: (id: ReconciliationId, date: string, amount: Cents) =>
		call<OpenEvent>('reconciliation_update', { id, date, amount }),
	deleteReconciliation: (id: ReconciliationId) => call<OpenEvent>('reconciliation_delete', { id }),
	addPerson: (name: string, person: Person | null = null) =>
		call<Added>('person_add', { name, person }),
	updatePerson: (id: AccountId, name: string, person: Person) =>
		call<OpenEvent>('person_update', { id, name, person }),
	checkIban: (input: string) => call<CheckedIban>('iban_check', { input }),

	addBudgetLine: (draft: BudgetLineDraft) => call<OpenEvent>('budget_line_add', { draft }),
	updateBudgetLine: (id: BudgetLineId, draft: BudgetLineDraft) =>
		call<OpenEvent>('budget_line_update', { id, draft }),
	/** The linked actuals stay and count as unbudgeted. */
	deleteBudgetLine: (id: BudgetLineId) => call<OpenEvent>('budget_line_delete', { id }),

	addAssumption: (name: string, value: number) =>
		call<OpenEvent>('assumption_add', { name, value }),
	/** Every budget line that scales with it follows. */
	updateAssumption: (id: AssumptionId, name: string, value: number) =>
		call<OpenEvent>('assumption_update', { id, name, value }),
	/** Refused while a budget line scales with it. */
	deleteAssumption: (id: AssumptionId) => call<OpenEvent>('assumption_delete', { id }),

	addCostCentre: (name: string) => call<OpenEvent>('cost_centre_add', { name }),
	renameCostCentre: (id: CostCentreId, name: string) =>
		call<OpenEvent>('cost_centre_rename', { id, name }),
	/** What was tagged stays, without a tag. */
	deleteCostCentre: (id: CostCentreId) => call<OpenEvent>('cost_centre_delete', { id }),
	/** What a new line of the category starts with. */
	setCategoryCostCentre: (category: AccountId, costCentre: CostCentreId | null) =>
		call<OpenEvent>('category_set_cost_centre', { category, costCentre }),
	/** One plugin's changes, applied together (technical design §10). */
	applyPluginRecords: (plugin: string, changes: PluginChange[]) =>
		call<OpenEvent>('plugin_records_apply', { plugin, changes }),

	addTier: (draft: TierDraft) => call<OpenEvent>('tier_add', { draft }),
	updateTier: (id: TierId, draft: TierDraft) => call<OpenEvent>('tier_update', { id, draft }),
	/** Only a tier that no import and no manual sale uses. */
	deleteTier: (id: TierId) => call<OpenEvent>('tier_delete', { id }),
	/** Off is refused while there are tiers. */
	setTicketsEnabled: (enabled: boolean) => call<OpenEvent>('tickets_set_enabled', { enabled }),
	setTicketAccount: (account: AccountId | null) =>
		call<OpenEvent>('ticket_account_set', { account }),
	/** Reads a ticket sheet (path) with the event's columns; changes nothing. */
	previewTickets: (batch: TicketBatch, date: string) =>
		call<ImportPreview>('tickets_preview', { batch, date }),
	importTickets: (batch: TicketBatch, date: string) =>
		call<OpenEvent>('tickets_import', { batch, date }),
	/** The data sources the open event can pull from, by plugin: its own and this computer's. */
	dataSources: () => call<DataSources>('data_sources_get'),
	/**
	 * Saves a plugin's data source on this computer, or in the event file with `inEvent`; null
	 * removes it, and its token once nothing uses it. The token is never in the event file.
	 */
	setDataSource: (id: string, source: DataSource | null, inEvent = false) =>
		call<DataSources>('data_source_set', { id, source, inEvent }),
	/** The token of the source's provider. It is never handed back; null removes it. */
	setDataSourceToken: (id: string, token: string | null) =>
		call<DataSources>('data_source_set_token', { id, token }),
	/** Asks the provider for these fields, of which `optional` may be missing; changes nothing. */
	pullDataSource: (id: string, fields: string[], optional: string[] = []) =>
		call<Pulled>('data_source_pull', { id, fields, optional }),
	/** Reads these columns of a file; changes nothing. */
	readTable: (path: string, columns: string[]) => call<Table>('table_read', { path, columns }),
	setImportDate: (id: ImportId, date: string) => call<OpenEvent>('import_set_date', { id, date }),
	/** What deleting an import would change; changes nothing. */
	previewDeleteImport: (id: ImportId) => call<ImportDeletion>('import_delete_preview', { id }),
	/** Everything goes back to the sheets that are left. Refused while a cancellation needs it. */
	deleteImport: (id: ImportId) => call<OpenEvent>('import_delete', { id }),
	addManualSale: (draft: ManualSaleDraft) => call<OpenEvent>('manual_sale_add', { draft }),
	updateManualSale: (id: ManualSaleId, draft: ManualSaleDraft) =>
		call<OpenEvent>('manual_sale_update', { id, draft }),
	deleteManualSale: (id: ManualSaleId) => call<OpenEvent>('manual_sale_delete', { id }),
	addCancellation: (draft: CancellationDraft) => call<OpenEvent>('cancellation_add', { draft }),
	updateCancellation: (id: CancellationId, edit: CancellationEdit) =>
		call<OpenEvent>('cancellation_update', { id, edit }),
	/** Tickets it resolved are a conflict again. */
	deleteCancellation: (id: CancellationId) => call<OpenEvent>('cancellation_delete', { id }),
	resolveConflict: (order: string, tier: TierId, resolution: Resolution) =>
		call<OpenEvent>('conflict_resolve', { order, tier, resolution }),

	exportPayoutsCsv: (target: string) => call<void>('payouts_export_csv', { target }),
	exportTransactionsCsv: (target: string) => call<void>('transactions_export_csv', { target }),
	/** Closes the event (R-SET-7): every open item needs a note in `overrides`. */
	closeEvent: (overrides: BlockerOverride[]) => call<OpenEvent>('event_close', { overrides }),
	reopenEvent: () => call<OpenEvent>('event_reopen'),
	markPayoutsPaid: (account: AccountId, date: string, people: AccountId[]) =>
		call<OpenEvent>('payouts_mark_paid', { account, date, people }),
	/** The print dialog, e.g. to save the payout list as PDF. */
	print: async () => (isTauri() ? call<void>('print_window') : window.print())
};

/** Where the webview loads an attachment from, for previews (`lancelot://` in the app). */
export async function attachmentUrl(attachment: Attachment): Promise<string> {
	if (isTauri()) return convertFileSrc(`attachment/${attachment.id}`, 'lancelot');
	return (await import('./dev')).attachmentUrl(attachment.id);
}
